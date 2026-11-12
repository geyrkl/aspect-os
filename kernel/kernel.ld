/* ASPECT OS - kernel linker script: kernel at 0x2000000 (32 MiB).
   The UEFI memory map reserves low memory (OVMF boot services code spans
   0x0900000-0x1780000); the first big conventional region starts at
   0x1780000, so 32 MiB is a safe fixed load address and leaves room for
   the 32 MiB image span (code + ~19 MiB of .bss buffers). */
ENTRY(kmain)
SECTIONS
{
  . = 0x2000000;
  .text : {
    *(.text.first)
    *(.text .text.*)
  }
  .rodata : {
    *(.rodata .rodata.*)
  }
  .data : {
    *(.data .data.*)
  }
  .bss (NOLOAD) : {
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    __bss_end = .;
  }
}
