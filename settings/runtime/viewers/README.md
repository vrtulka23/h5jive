# Inspector view settings

Each `.dip` file supplies concrete values for one Data Inspector view. The
matching [view schema](../schemas/views/README.md) defines defaults, types,
and validation. The [runtime DIPfile](../DIPfile) registers these files as
named sources, and [`viewer.dip`](../viewer.dip) includes them under
`viewer.views`.
