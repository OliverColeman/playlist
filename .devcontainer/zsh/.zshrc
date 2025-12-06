# Configure history
HISTFILE=$SCRATCH/.zsh_history
HISTSIZE=100000
SAVEHIST=100000
setopt SHARE_HISTORY # Share history across all sessions
setopt APPEND_HISTORY # Append new history lines to the history file immediately
setopt HIST_IGNORE_DUPS    # Ignore duplicate commands in history
setopt PUSHD_IGNORE_DUPS   # Ignore duplicate directories in the directory stack

# Path to your antidote installation
export ZDOTDIR=$HOME
source /usr/share/zsh-antidote/antidote.zsh
antidote load $ZDOTDIR/.zsh_plugins.txt

bindkey "$terminfo[kcuu1]" history-substring-search-up
bindkey "$terminfo[kcud1]" history-substring-search-down

alias ll='ls -alF'
alias la='ls -A'
alias l='ls -CF'

# Git aliases (highly recommended)
alias gs='git status -s'
alias ga='git add'
alias gc='git commit'
alias gp='git push'
alias gd='git diff'
alias gl='git log --oneline --graph --all'
alias gco='git checkout'

alias cm='cargo make'

# Set your default editor
export EDITOR="nano"
# To customize prompt, run `p10k configure` or edit ~/.p10k.zsh.
[[ ! -f ~/.p10k.zsh ]] || source ~/.p10k.zsh
