MEMORY {
    FLASH : ORIGIN = DEFINED(__run_from_ram) ? 0x20000000 : 0x10000000,
            LENGTH = DEFINED(__run_from_ram) ? 256K : 4096K
    RAM   : ORIGIN = DEFINED(__run_from_ram) ? 0x20040000 : 0x20000000,
            LENGTH = DEFINED(__run_from_ram) ? 8K : 264K
}
