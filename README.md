# Magicportal

Magicportal allows you to forward multicast UDP data from one place to another where multicast is not supported.

Magicportal has to be run on both servers. It uses Gnatsd(https://nats.io) message queue for sending messages between them.

It can be run on two modes:

1. `forwarder`: Has to be run in this mode on the server where data has to be received.
2. `agent`: Has to be run in this mode on the server where multicast isn't supported.

> Note: TLS can be enabled, but currently `Magicportal` doesn't support client certificates. This will be suppported in the future.

Magicportal needs a config file named `config.json` in the same directory as Magicportal.

An example config file:

```json
{
  "multicast_groups": [
    {
      "multicast_addr": "233.1.2.5:34074",
      "interface": "lo"
    }
  ],
  "mode": "agent",
  "tls_enabled": false,
  "nats_url": "nats://127.0.0.1:4222",
  "send_as_unicast": true,
  "unicast_addrs": {
    "233.1.2.5:34074": "127.0.0.1:24113"
  },
  "max_packet_size": 1024
}
```

We can give multiple multicast groups in the `multicast_groups` array. `send_as_unicast` will tell the agent to send the data as unicast to the address mapped with the multicast group in `unicast_addrs`.