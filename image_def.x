_stext = 0x10000300;

SECTIONS {
    .start_block 0x10000200 : ALIGN(4)
    {
        KEEP(*(.start_block));
    } > FLASH
} INSERT BEFORE .text;
