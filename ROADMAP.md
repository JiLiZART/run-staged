## Roadmap

A CLI tool (inspired by nano-staged and lint-staged) to run commands for defined set of patterns in config for git staged files befor git push

[ ] Deep config scan using current pwd, look at files (use node node_modules resolution algorithm) `.run-staged.toml` or `run-staged.toml` or read `run-staged.json` or `.run-staged.json`
```
  timeout = "300ms"
  
  [group1]
  timeout = "1s"
  patterns = { '*.rs' = ['cargo fmt'], "*.{js,ts,tsx,jsx}" = ['npm run fmt'] }
  
  [group2]
  timeout = "5s"
  
  [group2.patterns]
  "*.css" = [
      "npm run prettier:staged",
      "npm run stylelint:staged"
  ]
  
  [group3.patterns]
  "**/*.rs" = [
      "cargo fmt",
      "cargo clippy",
  ]

```
[ ] Add ability to define `timeout` config option
[ ] Groups in config, each group has list of patterns to watch and own settings for group like execution order
[ ] Add ability to define execution order, parallel or sequential in each group
[ ] Add ability to define timeout of command in each group
[ ] Add ability to define command execution behavior, run on each file or pass a list of files to command in each group
[ ] Add ability to define continue execution of commands if any command fails
[ ] Display total affected files count
[ ] Display total time of execution
[ ] Display total time of execution per command
[ ] Add ability to define relative or absolute file paths will be passed to command for each group
[ ] Add config variations, read .run-staged.toml, or run-staged.toml or read run-staged.json or .run-staged.json
[ ] Add checks and readable errors (create errors enum) for `no config`, `config is not valid`, `no git repository`, `no staged files found` , `no files found matched for group_name patterns`, `failed to execute command, no command found`,
[ ] Catch process signals or keyboard interrupts and gracefully exit
[ ] Add run options --config path/to/config.toml
[ ] Add --unstaged to run commands for unstaged files too
[ ] Add --quiet for running all tasks without TUI and return exit code if run fails
[ ] Add support to backup staged files before tasks run as patch file in .git/run-staged.diff and delete file after
[ ] Add files chunking support, if in config set that not need to run program on each file need to create a chunk of files and pass files chunk on program run like `eslint file1.js file2.js` and for second chunk `eslint file3.js file4.js`, ad config option for this
[ ] Display running commands by groups when running for example
[ ] Add ability to define mutable or readonly group (for readonly means that commands dont modify files)

```
  - group1
    (*.js, *.ts) - file1.js
  - group2
    (*.rs) - toml.rs

```

[ ] Display errors for failed pattern runs like (stderr and sdout in red block with border)

```
  - group1
    (*.js, *.ts) - file1.js
      --------------------------------------
      |                                     |
      |   Error text here                   |
      |                                     |
      --------------------------------------
  - group2
    (*.rs) - toml.rs

```

[ ] Add cli --install comand to define git hook itself without using `husky` or whatever
[ ] Display rich log of whats going on when running tasks, like lint-staged do or do it in better way
```
⋯ Backing up original state…
✔ Done backing up original state (1f4c047d)!
⋯ Running tasks for staged files…
    *.{json,md} — 1 file
      ⋯ prettier --write

✔ prettier --write

✔ Done running tasks for staged files!
⋯ Staging changes from tasks…
✔ Done staging changes from tasks!
⋯ Cleaning up temporary files…
✔ Done cleaning up temporary files!
```
