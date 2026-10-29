---
title: Shell completion
description: Generate and enable Steward completions for Bash, Zsh, Fish, PowerShell, or Elvish.
---

`steward completions SHELL` writes a completion script to stdout. It works without proxy connections or configuration and includes the direct route commands and flags.

## Bash

Enable it for this session:

```sh
source <(steward completions bash)
```

For future sessions, add that line to your own Bash startup file after Steward is on `PATH`.

## Zsh

Initialize Zsh completion, then load the generated script:

```sh
autoload -Uz compinit
compinit
source <(steward completions zsh)
```

## Fish

```fish
steward completions fish | source
```

For persistent completion:

```sh
mkdir -p ~/.config/fish/completions
steward completions fish > ~/.config/fish/completions/steward.fish
```

## PowerShell

```powershell
steward completions powershell | Out-String | Invoke-Expression
```

## Elvish

```text
eval (steward completions elvish | slurp)
```

The installer does not edit shell startup files. Static completion covers command
names and options. In v0.2.0-alpha.2, add `--dynamic` for saved proxy/route IDs:

```sh
source <(steward completions bash --dynamic)
```

Dynamic completion reads saved local proxy/route IDs without contacting APIs or
creating state. Re-source it after upgrades. It does not use local IDs for an SSH
or container target.
