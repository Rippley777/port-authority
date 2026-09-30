# Source this file in bash or zsh. No startup files are modified automatically.
# Set PORT_AUTHORITY_BIN to the desktop executable before sourcing.
if [ -z "${BASH_VERSION-}${ZSH_VERSION-}" ]; then
  printf '%s\n' 'Port Authority shell integration requires bash or zsh.' >&2
  return 1
fi
pa() {
  if [ -z "${PORT_AUTHORITY_BIN-}" ] || [ ! -x "$PORT_AUTHORITY_BIN" ]; then
    printf '%s\n' 'Set PORT_AUTHORITY_BIN to the Port Authority executable. Copy setup from Conflict Autopilot.' >&2
    return 127
  fi
  "$PORT_AUTHORITY_BIN" --autopilot-run -- "$@"
}
# Optional session-only interception of conventional dev/start commands.
# Existing user functions are never replaced. Other commands bypass capture.
pa_autopilot_on() {
  if [ "${_PA_AUTOPILOT_ACTIVE-}" = 1 ]; then return 0; fi
  for _pa_name in npm pnpm yarn bun; do
    if typeset -f "$_pa_name" >/dev/null 2>&1 || alias "$_pa_name" >/dev/null 2>&1; then
      printf '%s\n' "Port Authority: $_pa_name is already a shell function or alias. Use 'pa $_pa_name …' with the external executable instead." >&2
      return 1
    fi
  done
  function npm { case "${1-} ${2-}" in 'run dev'|'run start'|'start '*) pa npm "$@" ;; *) command npm "$@" ;; esac; }
  function pnpm { case "${1-} ${2-}" in 'run dev'|'run start'|'dev '*|'start '*) pa pnpm "$@" ;; *) command pnpm "$@" ;; esac; }
  function yarn { case "${1-} ${2-}" in 'run dev'|'run start'|'dev '*|'start '*) pa yarn "$@" ;; *) command yarn "$@" ;; esac; }
  function bun { case "${1-} ${2-}" in 'run dev'|'run start') pa bun "$@" ;; *) command bun "$@" ;; esac; }
  _PA_NPM_BODY="$(typeset -f npm)"
  _PA_PNPM_BODY="$(typeset -f pnpm)"
  _PA_YARN_BODY="$(typeset -f yarn)"
  _PA_BUN_BODY="$(typeset -f bun)"
  _PA_AUTOPILOT_ACTIVE=1
  printf '%s\n' 'Port Authority is capturing launch metadata and watching dev/start conflicts in this shell. Use pa_autopilot_off to stop.'
}
pa_autopilot_off() {
  if [ "${_PA_AUTOPILOT_ACTIVE-}" = 1 ]; then
    [ "$(typeset -f npm)" != "$_PA_NPM_BODY" ] || unset -f npm
    [ "$(typeset -f pnpm)" != "$_PA_PNPM_BODY" ] || unset -f pnpm
    [ "$(typeset -f yarn)" != "$_PA_YARN_BODY" ] || unset -f yarn
    [ "$(typeset -f bun)" != "$_PA_BUN_BODY" ] || unset -f bun
    unset _PA_NPM_BODY _PA_PNPM_BODY _PA_YARN_BODY _PA_BUN_BODY
    unset _PA_AUTOPILOT_ACTIVE
  fi
}
