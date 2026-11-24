# View schemas

One schema per inspector view defines its setting types, defaults, and valid
ranges. Concrete values live in [`runtime/viewers/`](../../viewers/README.md).
`schemas/viewer.dip` connects the view subtrees to the main viewer schema, and
the parent [schema directory](../README.md) explains registration.
