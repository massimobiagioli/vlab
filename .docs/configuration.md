# Configuration

## Settings file

`settings.yaml` lives in the per-user configuration directory:

```yaml
provider: deepseek
model: "<supported-model-id>"
# Optional; GitHub is the only supported source-control provider.
source_control:
  provider: github
```

| Field | Rule |
|---|---|
| `provider` | `deepseek` only. |
| `model` | 1–64 characters of letters, digits, `.`, `_`, `:`, `-`; taken from the provider documentation. No default is hardcoded. |
| `source_control.provider` | Optional; `github` only; omitted means GitHub. |

Unknown fields, duplicate keys, multiple documents, and non-scalar values are
rejected. The file is capped at 64 KiB and never contains credentials.

## Credentials

| Variable | Use |
|---|---|
| `DEEPSEEK_API_KEY` | Planning and questions; sent only in the Authorization header. |
| `GITHUB_API_KEY` | Required when creating a project with `--github-source`; never stored or displayed. |

Both are read from the environment and are never written to settings, the
database, events, interaction metadata, prompts, logs, or command output.

## Paths

| Purpose | Default | Override |
|---|---|---|
| Settings | per-user config dir + `vlab/settings.yaml` | `VLAB_CONFIG_DIR` |
| Database | per-user data dir + `vlab/vlab.db` | `VLAB_DATA_DIR`, `VLAB_DB_PATH` |

Overrides must be absolute. Directories are created `0700`; files `0600` on
Unix. VLab refuses a directory readable by other users.

## Editor

`--edit-settings` uses `VISUAL`, then `EDITOR`, split with POSIX shell-word
rules. The editor is executed directly, never through a shell. Quote paths with
spaces, for example `EDITOR='"/Applications/My Editor" --wait'`.

## Development and tests

- `make run ARGS="..."` uses `.data/` for settings and database (ADR-0014).
- `VLAB_DEEPSEEK_ENDPOINT` is a test-only loopback override for the mock
  provider; non-loopback values are ignored.
- `RUST_LOG` enables stderr diagnostics; unset by default. Diagnostics contain
  no prompts, responses, memory content, or credentials.
