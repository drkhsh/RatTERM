# RatTERM

RatTERM is a fork of [IcyTERM](https://github.com/mkrueger/icy_tools) by Mike
Krüger that adds a **Reticulum (rnsh) connection type**, so you can dial BBSes
published on the [Reticulum](https://reticulum.network/) mesh the same way you
dial a Telnet or SSH board.

It also renames the binary to `ratterm` and drops upstream's update-check
banner. The other tools in this workspace ([Icy Draw](crates/icy_draw/README.md),
[Icy View](crates/icy_view/README.md), [Icy Play](crates/icy_play/README.md))
are untouched upstream code. The fork exists because the Reticulum transport
cannot go upstream; see [Why a fork?](#why-a-fork).

## Using it

1. In the dialing directory, set the connection type to **Reticulum (rnsh)**.
2. Put the 32-character hex **destination hash** in the address field. An
   `rns://` prefix is accepted and ignored.
3. Connect.

If a **Reticulum shared instance** is already running, RatTERM attaches to it
and routes over its interfaces; the reference Python `rnsd` uses the same local
socket, so its instance works too. Otherwise RatTERM brings up its own instance
from rsReticulum's config directory (`~/.config/rsReticulum/config` on Linux),
which has no interfaces until you add them. Either way, a working interface with
a path to the destination is required.

On first use a client identity is generated as `reticulum_identity` inside
IcyTERM's config directory, which RatTERM deliberately shares. To reach a
listener that restricts access, add that identity's hash to its allowed list
(`rnsh-rs -a <hash>`).

## Building

```sh
cargo build --release -p icy_term
# -> target/release/ratterm
```

The crate is still named `icy_term`; only the binary is renamed. Keeping
upstream's crate and paths intact is what makes rebasing onto IcyTERM cheap.

The `reticulum` feature is on by default. Without it you get plain IcyTERM:

```sh
cargo build --release -p icy_term --no-default-features
```

A `rust-toolchain.toml` pins the toolchain, because rsReticulum needs edition 2024.

## Why a fork?

The transport is built on [rsReticulum](https://github.com/ratspeak/rsReticulum),
which is **AGPL-3.0-or-later**. IcyTERM is MIT/Apache-2.0. Linking the two makes
the resulting binary AGPL, which upstream cannot accept for a project shipping
MIT/Apache release artifacts, so this is a downstream fork rather than a pull
request.

That direction is fine: MIT and Apache-2.0 are both one-way compatible with
AGPL-3.0, so a derivative may be distributed under AGPL. The reverse is not true,
which is exactly why upstream can't take it.

rsReticulum is also `publish = false` and describes itself as experimental, so
the workspace `Cargo.toml` pins it to a release tag.

## Licensing

- Upstream IcyTERM code remains under **MIT or Apache-2.0** (`LICENSE-MIT`,
  `LICENSE-APACHE`), unmodified.
- `icy_term` **built with the `reticulum` feature** links AGPL-3.0-or-later code,
  so *that binary* is **AGPL-3.0-or-later** when distributed.
- The other crates in this workspace are untouched and unaffected.

If you distribute builds, add the license text:

```sh
curl -o LICENSE-AGPL https://www.gnu.org/licenses/agpl-3.0.txt
```

Apache-2.0 section 4(b) requires stating that files were changed; the fork commits and
this file serve that purpose.

## Credits

IcyTERM is by **Mike Krüger**; all of the terminal, rendering, protocol and UI
work is his. RatTERM adds a transport and renames the result.

Reticulum is by **Mark Qvist**; the Rust implementation (rsReticulum) is by
**ratspeak**.
