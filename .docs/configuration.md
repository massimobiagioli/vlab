# Configuration

## Settings file

`settings.yaml` lives in the per-user configuration directory:

```yaml
provider: deepseek
# Current DeepSeek models: deepseek-flash (recommended) or deepseek-v4-pro.
model: "deepseek-flash"
# Optional; GitHub is the only supported source-control provider.
source_control:
  provider: github
credentials:
  deepseek_api_key: "sk-..."
  github_api_key: "ghp-..."   # optional
```

| Field | Rule |
|---|---|
| `provider` | `deepseek` only. |
| `model` | 1–64 characters of letters, digits, `.`, `_`, `:`, `-`; taken from the provider documentation. No default is hardcoded. |
| `source_control.provider` | Optional; `github` only; omitted means GitHub. |
| `credentials.deepseek_api_key` | Optional token, required for planning and questions; at most 512 bytes. |
| `credentials.github_api_key` | Optional token, required only for `--github-source`; at most 512 bytes. |

Unknown fields, duplicate keys, multiple documents, and non-scalar values are
rejected. The file is capped at 64 KiB.

## Credentials

`settings.yaml` is the only credential source. VLab never reads
`DEEPSEEK_API_KEY` or `GITHUB_API_KEY` from the environment. The key is sent
only in the Authorization header of the provider request; the GitHub key is
used only to confirm that a GitHub-backed project may be created. Neither is
written to the database, events, interaction metadata, prompts, logs, errors,
or command output, and `--show-settings` reports only `set`/`unset`.

Because the file contains secrets:

- keep it outside version control; it is already ignored by `.gitignore`;
- keep it private: VLab writes it with mode `0600`, refuses an unsafe
  directory (`0700` is created and verified), and on Unix refuses to load a
  file readable by group or other users;
- do not copy it into backups, tickets, chats, or screenshots without
  protection.

A placeholder example is tracked at `.data/settings.example.yaml` for the
development data directory. Create the private file with
`install -m 600 .data/settings.example.yaml .data/settings.yaml`, or let
`--edit-settings` create it.

## Paths

| Purpose | Default | Override |
|---|---|---|
| Settings | per-user config dir + `vlab/settings.yaml` | `VLAB_CONFIG_DIR` |
| Database | per-user data dir + `vlab/vlab.db` | `VLAB_DATA_DIR`, `VLAB_DB_PATH` |

A debug build run from `<repo>/target/debug/vlab` uses `<repo>/.data` for both
settings and database when that directory exists and no override is set
(ADR-0031). Release and installed binaries use the per-user defaults.

Overrides must be absolute. Directories are created `0700`; files `0600` on
Unix. VLab refuses a directory readable by other users.

## Editor

`--edit-settings` uses `VISUAL`, then `EDITOR`, split with POSIX shell-word
rules. The editor is executed directly, never through a shell. Quote paths with
spaces, for example `EDITOR='"/Applications/My Editor" --wait'`.

## Development and tests

- `make run ARGS="..."` uses `.data/` for settings and database (ADR-0014);
  a direct debug build from the checkout does the same (ADR-0031).
- `VLAB_DEEPSEEK_ENDPOINT` is a test-only loopback override for the mock
  provider; non-loopback values are ignored.
- `RUST_LOG` enables stderr diagnostics; unset by default. Diagnostics contain
  no prompts, responses, memory content, or credentials.
