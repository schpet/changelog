# changelog

a command line tool for managing changelogs following the [keep a changelog](https://keepachangelog.com) format.

## why

i like the idea of keeping a changelog but it's a bit of a pain. you need to know what changed, know what version you are on, know how to compare the changes between those, know the [keep a changelog format][kac], manually add the right markdown headers and links for each release.

this tool is aims to make it easy and fun to keep your changelog up to date. it offers commands to review your changes in an interface similar to a git interactive rebase, allowing you to select and reword commit messages into meaningful changelog entries. there's a comand to add `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, and `Security` changes in a structured way. and a command to make a release with easy semver bumping.

[kac]: https://keepachangelog.com/en/1.1.0/

## install

### homebrew

```
brew install schpet/tap/changelog
```

### binaries

[https://github.com/schpet/changelog/releases/latest](https://github.com/schpet/changelog/releases/latest)

### shell completions

generate and install shell completions:

```bash
# bash
changelog completions bash > ~/.local/share/bash-completion/completions/changelog

# zsh
changelog completions zsh > ~/.zsh/completions/_changelog  # ensure dir exists and is in fpath

# fish
source (changelog completions fish | psub) # in ~/.config/fish/config.fish

# alternatively:
changelog completions fish > ~/.config/fish/completions/changelog.fish
```

## usage

manage your project's changelog from the command line.

### adding entries

add a new entry to the unreleased section:

```
$ changelog add "new api endpoint for users" --type added
+ ### Added
+ - new api endpoint for users

$ changelog add "improved error messages" --type changed
+ ### Changed
+ - improved error messages

$ changelog add "fixed login bug" --type fixed --version 1.0.1
+ ### Fixed
+ - fixed login bug
```

### releasing versions

release the unreleased section to a new version:

```
# automatically increment the version
$ changelog release major  # 1.0.0 -> 2.0.0
$ changelog release minor  # 1.0.0 -> 1.1.0
$ changelog release patch  # 1.0.0 -> 1.0.1

# or specify an explicit version
$ changelog release 1.0.0
Released version 1.0.0

$ changelog release 1.0.0 --date 2025-01-01
Released version 1.0.0
```

### reviewing changes

interactively review git commits and add them to the changelog (similar to `git rebase -i`):

```
$ changelog review
Select commits to include in changelog (press 'a' to select all):
> [ ] abc1234 add user authentication
  [ ] def5678 fix typo in docs
  [ ] ghi9012 update dependencies
```

After selecting commits, you'll be dropped into your editor to categorize and reword the changes, just like an interactive rebase.

### version information

get version information:

```
$ changelog version latest
1.0.0

$ changelog version list
1.0.0
0.9.0
0.8.0

$ changelog version range 1.0.0
v0.9.0..v1.0.0
```

### other commands

show a specific version's entries:

```
$ changelog entry 1.0.0
## [1.0.0] - 2025-01-01

### Added
- Initial release
```

format the changelog:

```
$ changelog fmt
Formatted CHANGELOG.md
```

initialize a new changelog:

```
$ changelog init
Created CHANGELOG.md
```
