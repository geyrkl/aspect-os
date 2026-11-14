# ASPECT OS - own VNC (RFB) viewer for the live test drive.
# QEMU runs headless inside WSL and serves RFB on 127.0.0.1:5901; this
# script connects from Windows with a small embedded C# client (WinForms,
# no external dependencies). VNC pointer events are absolute, so the
# guest mouse works reliably - WSLg window capture is bypassed entirely.
#
# usage:  powershell -ExecutionPolicy Bypass -File scripts\vnc-viewer.ps1
#             [-VncHost 127.0.0.1] [-VncPort 5901] [-Fullscreen]
#             [-Test <path.bmp>]   # headless self-test: grab one frame, save, exit
param(
    [string]$VncHost = "127.0.0.1",
    [int]$VncPort = 5901,
    [switch]$Fullscreen,
    [string]$Test = ""
)

$src = @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

// Minimal RFB 3.8 client: None security, raw encoding only. Enough for
// QEMU on localhost - this is our own code, like everything in ASPECT OS.
namespace AspectVnc {
public class Client : IDisposable {
    NetworkStream s;
    public int W, H;

    static byte[] ReadExact(NetworkStream st, int n) {
        byte[] b = new byte[n];
        int off = 0;
        while (off < n) {
            int got = st.Read(b, off, n - off);
            if (got <= 0) throw new Exception("connection closed");
            off += got;
        }
        return b;
    }

    public Client(string host, int port) {
        TcpClient tcp = new TcpClient();
        tcp.Connect(host, port);
        tcp.NoDelay = true;
        s = tcp.GetStream();

        // handshake: version
        ReadExact(s, 12);
        byte[] hello = System.Text.Encoding.ASCII.GetBytes("RFB 003.008\n");
        s.Write(hello, 0, 12);

        // security types -> choose None (1)
        int n = s.ReadByte();
        if (n <= 0) throw new Exception("server refused connection");
        byte[] types = ReadExact(s, n);
        bool none = false;
        for (int i = 0; i < n; i++) { if (types[i] == 1) none = true; }
        if (!none) throw new Exception("server requires authentication");
        byte[] pick = new byte[1]; pick[0] = 1;
        s.Write(pick, 0, 1);
        byte[] res = ReadExact(s, 4);
        if (res[0] != 0 || res[1] != 0 || res[2] != 0 || res[3] != 0)
            throw new Exception("authentication failed");

        // ClientInit: shared
        byte[] ci = new byte[1]; ci[0] = 1;
        s.Write(ci, 0, 1);

        // ServerInit
        byte[] si = ReadExact(s, 2 + 2 + 16 + 4);
        W = (si[0] << 8) | si[1];
        H = (si[2] << 8) | si[3];
        int namelen = (si[20] << 24) | (si[21] << 16) | (si[22] << 8) | si[23];
        if (namelen > 0) ReadExact(s, namelen);

        // SetPixelFormat: 32bpp little-endian, true colour, R16 G8 B0
        byte[] pf = new byte[20];
        pf[4] = 32; pf[5] = 24; pf[6] = 0; pf[7] = 1;
        pf[8] = 0; pf[9] = 255; pf[10] = 0; pf[11] = 255; pf[12] = 0; pf[13] = 255;
        pf[14] = 16; pf[15] = 8; pf[16] = 0;
        s.Write(pf, 0, 20);
        // no SetEncodings message: the default encoding is raw
    }

    public void Request(bool incremental, int x, int y, int w, int h) {
        byte[] m = new byte[10];
        m[0] = 3;
        m[1] = (byte)(incremental ? 1 : 0);
        m[2] = (byte)(x >> 8); m[3] = (byte)x;
        m[4] = (byte)(y >> 8); m[5] = (byte)y;
        m[6] = (byte)(w >> 8); m[7] = (byte)w;
        m[8] = (byte)(h >> 8); m[9] = (byte)h;
        s.Write(m, 0, 10);
    }

    public void Pointer(int x, int y, int buttons) {
        byte[] m = new byte[6];
        m[0] = 5; m[1] = (byte)buttons;
        m[2] = (byte)(x >> 8); m[3] = (byte)x;
        m[4] = (byte)(y >> 8); m[5] = (byte)y;
        s.Write(m, 0, 6);
    }

    public void Key(uint keysym, bool down) {
        byte[] m = new byte[8];
        m[0] = 4; m[1] = (byte)(down ? 1 : 0);
        m[4] = (byte)(keysym >> 24); m[5] = (byte)(keysym >> 16);
        m[6] = (byte)(keysym >> 8); m[7] = (byte)keysym;
        s.Write(m, 0, 8);
    }

    public class Update { public int X, Y, W2, H2; public byte[] Px; }

    // read one FramebufferUpdate message (skips bell / cut text / colour map)
    public Update[] ReadUpdate() {
        while (true) {
            int type = s.ReadByte();
            if (type == 0) {
                byte[] h = ReadExact(s, 3);
                int n = (h[1] << 8) | h[2];
                Update[] us = new Update[n];
                for (int i = 0; i < n; i++) {
                    byte[] rh = ReadExact(s, 12);
                    int x = (rh[0] << 8) | rh[1];
                    int y = (rh[2] << 8) | rh[3];
                    int w = (rh[4] << 8) | rh[5];
                    int hh = (rh[6] << 8) | rh[7];
                    int enc = (rh[8] << 24) | (rh[9] << 16) | (rh[10] << 8) | rh[11];
                    if (enc != 0) throw new Exception("unexpected encoding " + enc);
                    Update u = new Update();
                    u.X = x; u.Y = y; u.W2 = w; u.H2 = hh;
                    u.Px = ReadExact(s, w * hh * 4);
                    us[i] = u;
                }
                return us;
            } else if (type == 1) {           // SetColourMapEntries
                byte[] h = ReadExact(s, 5);
                int ncol = (h[3] << 8) | h[4];
                ReadExact(s, ncol * 6);
            } else if (type == 2) {           // Bell
            } else if (type == 3) {           // ServerCutText
                byte[] h = ReadExact(s, 7);
                int len = (h[4] << 24) | (h[5] << 16) | (h[6] << 8) | h[7];
                if (len > 0) ReadExact(s, len);
            } else {
                throw new Exception("unexpected message type " + type);
            }
        }
    }

    public void Dispose() {
        if (s != null) s.Dispose();
    }
}

// scaled, double-buffered canvas for the guest framebuffer
public class Canvas : Control {
    public Bitmap Bmp;
    public object Lck = new object();
    public Rectangle Dest;

    public Canvas() {
        DoubleBuffered = true;
        ResizeRedraw = true;
        // hide the Windows cursor; the guest draws its own pointer
        Bitmap hid = new Bitmap(8, 8);
        Cursor = new Cursor(hid.GetHicon());
        SetStyle(ControlStyles.AllPaintingInWmPaint, true);
    }

    public void LayoutGuest() {
        if (Bmp == null) return;
        double sc = Math.Min((double)ClientSize.Width / Bmp.Width,
                             (double)ClientSize.Height / Bmp.Height);
        if (sc <= 0) return;
        int w = (int)(Bmp.Width * sc);
        int h = (int)(Bmp.Height * sc);
        Dest = new Rectangle((ClientSize.Width - w) / 2, (ClientSize.Height - h) / 2, w, h);
    }

    protected override void OnResize(EventArgs e) {
        base.OnResize(e);
        LayoutGuest();
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e) {
        if (Bmp == null) { e.Graphics.Clear(Color.FromArgb(16, 16, 20)); return; }
        lock (Lck) {
            e.Graphics.Clear(Color.FromArgb(16, 16, 20));
            e.Graphics.DrawImage(Bmp, Dest);
        }
    }

    // map a control-space point back to framebuffer coordinates
    public Point Map(Point p, int fw, int fh) {
        if (Dest.Width == 0 || Dest.Height == 0) return new Point(0, 0);
        int x = (int)((p.X - Dest.X) * fw / (double)Dest.Width);
        int y = (int)((p.Y - Dest.Y) * fh / (double)Dest.Height);
        if (x < 0) x = 0; if (x >= fw) x = fw - 1;
        if (y < 0) y = 0; if (y >= fh) y = fh - 1;
        return new Point(x, y);
    }
}

public class Viewer : Form {
    Client c;
    Canvas cv;
    Bitmap bmp;
    Thread rd;
    volatile bool run = true;
    int btns = 0;
    bool shDown = false, ctDown = false, alDown = false;
    bool full = false;

    static readonly Dictionary<Keys, uint> SYM = new Dictionary<Keys, uint>();
    static void FillSym() {
        SYM[Keys.Enter] = 0xFF0D;  SYM[Keys.Back] = 0xFF08;
        SYM[Keys.Escape] = 0xFF1B; SYM[Keys.Tab] = 0xFF09;
        SYM[Keys.Left] = 0xFF51;   SYM[Keys.Up] = 0xFF52;
        SYM[Keys.Right] = 0xFF53;  SYM[Keys.Down] = 0xFF54;
        SYM[Keys.Home] = 0xFF50;   SYM[Keys.End] = 0xFF57;
        SYM[Keys.PageUp] = 0xFF55; SYM[Keys.PageDown] = 0xFF56;
        SYM[Keys.Delete] = 0xFFFF; SYM[Keys.Insert] = 0xFF63;
    }

    static readonly string LogPath = System.IO.Path.Combine(
        System.IO.Path.GetTempPath(), "aspect-viewer.log");
    public static void Log(string msg) {
        try {
            System.IO.File.AppendAllText(LogPath,
                DateTime.Now.ToString("HH:mm:ss.fff") + " " + msg + "\r\n");
        } catch (Exception) { }
    }

    public Viewer(string host, int port, bool fullscreen) {
        FillSym();
        Log("viewer ctor");
        Text = "ASPECT OS (F11 = fullscreen, Alt+F4 = quit)";
        KeyPreview = true;
        StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(16, 16, 20);

        cv = new Canvas();
        cv.Dock = DockStyle.Fill;
        Controls.Add(cv);

        c = new Client(host, port);
        bmp = new Bitmap(c.W, c.H, PixelFormat.Format32bppRgb);
        cv.Bmp = bmp;

        // fit the window to the working area
        Rectangle wa = Screen.FromControl(this).WorkingArea;
        double sc = Math.Min((wa.Width - 40) / (double)c.W, (wa.Height - 90) / (double)c.H);
        ClientSize = new Size((int)(c.W * sc), (int)(c.H * sc));
        if (fullscreen) ToggleFull();

        cv.MouseDown += OnDown;
        cv.MouseUp += OnUp;
        cv.MouseMove += OnMove;
        KeyDown += OnKeyDown;
        KeyUp += OnKeyUp;
        KeyPress += OnKeyPress;
        FormClosed += OnClosed2;

        c.Request(false, 0, 0, c.W, c.H);
        Log("connected " + c.W + "x" + c.H);
        rd = new Thread(Reader);
        rd.IsBackground = true;
        rd.Start();
    }

    protected override void OnFormClosing(FormClosingEventArgs e) {
        Log("form closing: " + e.CloseReason.ToString());
        base.OnFormClosing(e);
    }

    void ToggleFull() {
        full = !full;
        if (full) {
            FormBorderStyle = FormBorderStyle.None;
            WindowState = FormWindowState.Maximized;
        } else {
            FormBorderStyle = FormBorderStyle.Sizable;
            WindowState = FormWindowState.Normal;
            Rectangle wa = Screen.FromControl(this).WorkingArea;
            double sc = Math.Min((wa.Width - 40) / (double)c.W, (wa.Height - 90) / (double)c.H);
            ClientSize = new Size((int)(c.W * sc), (int)(c.H * sc));
        }
    }

    void Reader() {
        try {
            Log("reader started");
            while (run) {
                Client.Update[] us = c.ReadUpdate();
                // same lock as Canvas.OnPaint: DrawImage and LockBits must
                // never touch the bitmap at the same time (GDI+ throws
                // "object is in use elsewhere" and the window would close)
                lock (cv.Lck) {
                    for (int i = 0; i < us.Length; i++) Blit(us[i]);
                }
                try { cv.BeginInvoke((MethodInvoker)delegate { cv.Invalidate(); }); }
                catch (ObjectDisposedException) { break; }
                catch (InvalidOperationException) { break; }
                c.Request(true, 0, 0, c.W, c.H);
            }
            Log("reader loop ended normally");
        } catch (Exception ex) {
            Log("reader exception: " + ex.ToString());
            if (run) {
                run = false;
                try { BeginInvoke((MethodInvoker)delegate { Close(); }); } catch (Exception) { }
            }
        }
    }

    void Blit(Client.Update u) {
        Rectangle r = new Rectangle(u.X, u.Y, u.W2, u.H2);
        if (r.Width <= 0 || r.Height <= 0) return;
        BitmapData bd = bmp.LockBits(r, ImageLockMode.WriteOnly, PixelFormat.Format32bppRgb);
        try {
            int srcStride = u.W2 * 4;
            if (bd.Stride == srcStride) {
                Marshal.Copy(u.Px, 0, bd.Scan0, u.Px.Length);
            } else {
                for (int row = 0; row < u.H2; row++) {
                    IntPtr dst = new IntPtr(bd.Scan0.ToInt64() + row * bd.Stride);
                    Marshal.Copy(u.Px, row * srcStride, dst, srcStride);
                }
            }
        } finally {
            bmp.UnlockBits(bd);
        }
    }

    void SendPointer(Point p) {
        Point m = cv.Map(p, c.W, c.H);
        c.Pointer(m.X, m.Y, btns);
    }

    void OnDown(object o, MouseEventArgs e) {
        if (e.Button == MouseButtons.Left) btns |= 1;
        if (e.Button == MouseButtons.Middle) btns |= 2;
        if (e.Button == MouseButtons.Right) btns |= 4;
        SendPointer(e.Location);
    }
    void OnUp(object o, MouseEventArgs e) {
        if (e.Button == MouseButtons.Left) btns &= ~1;
        if (e.Button == MouseButtons.Middle) btns &= ~2;
        if (e.Button == MouseButtons.Right) btns &= ~4;
        SendPointer(e.Location);
    }
    void OnMove(object o, MouseEventArgs e) {
        SendPointer(e.Location);
    }

    void OnKeyDown(object o, KeyEventArgs e) {
        if (e.KeyCode == Keys.F11) { ToggleFull(); e.Handled = true; return; }
        if (e.Shift && !shDown) { shDown = true; c.Key(0xFFE1, true); }
        if (e.Control && !ctDown) { ctDown = true; c.Key(0xFFE3, true); }
        if (e.Alt && !alDown) { alDown = true; c.Key(0xFFE9, true); }
        uint sym;
        if (SYM.TryGetValue(e.KeyCode, out sym)) {
            c.Key(sym, true);
            e.Handled = true;
        } else if (e.KeyCode >= Keys.F1 && e.KeyCode <= Keys.F12) {
            c.Key((uint)(0xFFBE + ((int)e.KeyCode - (int)Keys.F1)), true);
            e.Handled = true;
        }
    }
    void OnKeyUp(object o, KeyEventArgs e) {
        if (!e.Shift && shDown) { shDown = false; c.Key(0xFFE1, false); }
        if (!e.Control && ctDown) { ctDown = false; c.Key(0xFFE3, false); }
        if (!e.Alt && alDown) { alDown = false; c.Key(0xFFE9, false); }
        uint sym;
        if (SYM.TryGetValue(e.KeyCode, out sym)) {
            c.Key(sym, false);
            e.Handled = true;
        } else if (e.KeyCode >= Keys.F1 && e.KeyCode <= Keys.F12) {
            c.Key((uint)(0xFFBE + ((int)e.KeyCode - (int)Keys.F1)), false);
            e.Handled = true;
        }
    }
    void OnKeyPress(object o, KeyPressEventArgs e) {
        char ch = e.KeyChar;
        if (ch >= 32 && ch <= 126 || ch >= 160) {
            c.Key((uint)ch, true);
            c.Key((uint)ch, false);
        }
        e.Handled = true;
    }

    void OnClosed2(object o, FormClosedEventArgs e) {
        run = false;
        try { rd.Join(400); } catch (Exception) { }
        c.Dispose();
    }
}

public static class App {
    // test mode: connect, grab one full frame, save a BMP, exit (no GUI)
    static int TestFrame(string host, int port, string path) {
        Client c = new Client(host, port);
        Bitmap bmp = new Bitmap(c.W, c.H, PixelFormat.Format32bppRgb);
        c.Request(false, 0, 0, c.W, c.H);
        bool gotFull = false;
        int guard = 0;
        while (!gotFull && guard < 20) {
            guard++;
            Client.Update[] us = c.ReadUpdate();
            for (int i = 0; i < us.Length; i++) {
                Client.Update u = us[i];
                if (u.W2 == c.W && u.H2 == c.H) gotFull = true;
                BitmapData bd = bmp.LockBits(new Rectangle(u.X, u.Y, u.W2, u.H2),
                                             ImageLockMode.WriteOnly, PixelFormat.Format32bppRgb);
                int srcStride = u.W2 * 4;
                if (bd.Stride == srcStride) {
                    Marshal.Copy(u.Px, 0, bd.Scan0, u.Px.Length);
                } else {
                    for (int row = 0; row < u.H2; row++) {
                        IntPtr dst = new IntPtr(bd.Scan0.ToInt64() + row * bd.Stride);
                        Marshal.Copy(u.Px, row * srcStride, dst, srcStride);
                    }
                }
                bmp.UnlockBits(bd);
            }
            c.Request(true, 0, 0, c.W, c.H);
        }
        bmp.Save(path, ImageFormat.Bmp);
        bmp.Dispose();
        c.Dispose();
        Console.WriteLine("frame " + c.W + "x" + c.H + " saved to " + path);
        return 0;
    }

    public static int Run(string host, int port, bool fullscreen, string test) {
        try {
            Viewer.Log("app run start (test=" + (test == null ? "" : test) + ")");
            if (test != null && test.Length > 0) return TestFrame(host, port, test);
            Application.EnableVisualStyles();
            Application.Run(new Viewer(host, port, fullscreen));
            Viewer.Log("application run returned");
            return 0;
        } catch (Exception ex) {
            Viewer.Log("app exception: " + ex.ToString());
            Console.Error.WriteLine("vnc-viewer: " + ex.Message);
            try { MessageBox.Show(ex.Message, "ASPECT OS viewer"); } catch (Exception) { }
            return 1;
        }
    }
}
}
'@

Add-Type -ReferencedAssemblies System.Windows.Forms, System.Drawing, System, System.Core -TypeDefinition $src
[AspectVnc.App]::Run($VncHost, $VncPort, [bool]$Fullscreen, $Test)
