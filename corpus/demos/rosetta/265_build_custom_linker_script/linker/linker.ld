/* Minimal linker script for demo 265. */
ENTRY(_start)

SECTIONS
{
  .text : { *(.text*) }
}
