/* ASPECT OS - Kernel-Linker-Skript: Kernel bei 0x100000 (1 MiB) */
ENTRY(kmain)
SECTIONS
{
  . = 0x100000;
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
