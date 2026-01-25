# aish-stdio-to-uds

Traditionally, there are two transport mechanisms for an MCP server: stdio and HTTP.

This crate helps enable a third, which is UNIX domain socket, because it has the advantages that:

- The UDS can be attached to long-running process, like an HTTP server.
- The UDS can leverage UNIX file permissions to restrict access.

To that end, this crate provides an adapter between a UDS and stdio. The idea is that someone could start an MCP server that communicates over `/tmp/mcp.sock`. Then the user could specify this on the fly like so:

```
aish --config mcp_servers.example={command="aish-stdio-to-uds",args=["/tmp/mcp.sock"]}
```
