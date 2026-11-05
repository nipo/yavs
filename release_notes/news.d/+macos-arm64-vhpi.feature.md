VHPI plugins load on macOS on Apple silicon: the `yavs` executable exports the `vhpi_*` functions, and a plugin built with `-undefined dynamic_lookup` binds to them.
