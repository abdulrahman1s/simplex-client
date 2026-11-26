# Channel interop: the live run

`tests/it/channel_live.rs` checks channels against official SimpleX binaries. Its tests are
`#[ignore]`d. This file records how they were run and what they gave.

## Last run: 2026-11-26

Tree: `5e70aef` plus the uncommitted changes that add relay tests, adding relays to a channel,
and the replay checks (the Unreleased section of `CHANGELOG.md`).

### Binaries

These are the official release assets for x86_64 Linux (Ubuntu 22.04 builds), unmodified. Each
sha256 matches the digest GitHub publishes for the asset.

| Binary | Reports | sha256 |
|---|---|---|
| `simplex-chat` (simplex-chat v7.1.4, `62ea5dc4`) | `SimpleX Chat v7.1.4.0` | `344e52fb4794bf86b0ec01534692f9b930de2c36b3af21dad45045e5af59f22f` |
| `smp-server` (simplexmq v7.1.0 = `fdbcb24d`) | `SMP server v7.1.0.18 / fdbcb24` | `d4eac837d22e5e1d4e622d54af36ad51a195a259a609163bfae860f634472a01` |
| `xftp-server` (simplexmq v7.1.0) | `SimpleX XFTP server v7.1.0.18 / fdbcb24` | `47e9151d8cbe2ae5815c063fb7fd624f2e115efe2a7a7f9d6287be4d8c5b4aee` |

The binaries ran under the Nix glibc loader with Nix `openssl`, `zlib` and `gmp`, inside
`bwrap`. The servers' fixed paths and each CLI's `HOME` were mapped to a scratch folder.

### Network

Every process ran in one user and network namespace whose only interface is `lo`, so nothing
could reach a public or preset server. Each relay was told to make its address on the local
server.

```sh
unshare -rn sh -c 'ip link set lo up; exec sleep infinity' &     # the namespace, held by $NS
ns() { nsenter -t $NS -U -n --preserve-credentials "$@"; }
```

### Servers

```sh
ns smp-server init -l --ip 127.0.0.1 -y
# smp-server.ini: port = 5223; [WEB] https, cert and key commented out
ns smp-server start &
ns xftp-server init --ip 127.0.0.1 -p $STATE/xftp/files -q 1gb
# file-server.ini: port = 5443
ns xftp-server start &
SMP='smp://<fingerprint>:<create_password>@127.0.0.1:5223'
XFTP='xftp://<fingerprint>@127.0.0.1:5443'
```

### Official CLIs

There were two relays, an owner and a subscriber. The relays print their `/r#` address on
their first start.

```sh
ns simplex-chat -d $STATE/relay1/db -s "$SMP" --xftp-server "$XFTP" \
  --relay --headless --user-display-name relay1 --relay-address-server "$SMP" &
ns simplex-chat -d $STATE/relay2/db -s "$SMP" --xftp-server "$XFTP" \
  --relay --headless --user-display-name relay2 --relay-address-server "$SMP" &
ns simplex-chat -d $STATE/owner/db -s "$SMP" --xftp-server "$XFTP" -p 5301 --user-display-name owner &
ns simplex-chat -d $STATE/sub1/db  -s "$SMP" --xftp-server "$XFTP" -p 5302 --user-display-name sub1 &
```

The CLI hands each WebSocket event to only one connected client, so nothing else stayed
connected to ports 5301 and 5302 while the tests ran.

### Tests

```sh
cargo test --test it --no-run
ns env SIMPLEX_TEST_SMP="$SMP" \
  SIMPLEX_TEST_RELAY='<relay1 /r# address>' SIMPLEX_TEST_RELAY2='<relay2 /r# address>' \
  SIMPLEX_TEST_CLI_OWNER=ws://127.0.0.1:5301 SIMPLEX_TEST_CLI_SUB=ws://127.0.0.1:5302 \
  target/debug/deps/it-<hash> channel_live --ignored --test-threads 1
```

| Test | Result |
|---|---|
| `an_official_owner_s_channel_reaches_a_crate_subscriber_with_history_and_no_pq` | pass |
| `a_crate_owner_s_channel_reaches_an_official_subscriber_who_may_not_post` (includes `/relay test` against the official relay, and against an address that is no relay's) | pass |
| `two_crate_clients_share_a_channel_through_an_official_relay` (includes an empty-fallback post that is not replayed) | pass |
| `a_channel_on_two_relays_replays_the_last_100_posts_once_each_as_they_now_are` | pass |
| `a_relay_added_to_a_live_channel_is_announced_to_subscribers_and_joined_by_later_ones` | pass |

`test result: ok. 5 passed; 0 failed` (12.75 s), run on the final tree, after the gate.

### PQ on every leg

The crate's side of each leg is asserted in the tests: every `ChannelRelay::pq` the owner and
subscribers saw was `false`.

The official side was read from each CLI's own databases after the run. Each row counts every
member connection that CLI had, over this run and the earlier ones on the same databases:

| CLI | Member connections (peers) | chat `pq_support` / `pq_encryption` on | `pq_snd_enabled` / `pq_rcv_enabled` | agent `pq_support` on | ratchets with a KEM |
|---|---|---|---|---|---|
| relay1 | 88 (46 owners, 42 subscribers) | 0 / 0 | 0 / 0 | 0 | 0 |
| relay2 | 36 (17 owners, 19 subscribers) | 0 / 0 | 0 / 0 | 0 | 0 |
| owner | 13 (relays) | 0 / 0 | 0 / 0 | 0 | 0 |
| sub1 | 9 (relays) | 0 / 0 | 0 / 0 | 0 | 0 |

So no owner↔relay or relay↔subscriber leg had PQ, whichever implementation was on each end.
`ChannelRelay::pq`'s rustdoc gives the reason from the 7.1.4 and `fdbcb24d` sources.
