# changelog

a command line tool for managing changelogs following the [keep a changelog](https://keepachangelog.com) format.

## install

todo

## usage

manage your project's changelog from the command line.

### adding entries

add a new entry to the unreleased section:

```
$ changelog add "new api endpoint for users"
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
$ changelog release --version 1.0.0
Released version 1.0.0

$ changelog release --version 1.0.0 --date 2025-01-01
Released version 1.0.0

# or automatically increment the version
$ changelog release major  # 1.0.0 -> 2.0.0
$ changelog release minor  # 1.0.0 -> 1.1.0
$ changelog release patch  # 1.0.0 -> 1.0.1
```

### reviewing changes

interactively review git commits and add them to the changelog:

```
$ changelog review
? Select commits to include in changelog
> ⬡ abc1234 add user authentication
  ⬡ def5678 fix typo in docs
  ⬡ ghi9012 update dependencies
```

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
