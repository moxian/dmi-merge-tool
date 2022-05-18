# What is this?

A small tool for resolving git merge conflicts of DreamMaker's .dmi files.

# How do I use this

0) BACK UP YOUR WORK! This was written in an evening, and is likely very buggy.
1) Invoke `git merge` or similar, to obtain the conflicts. Example
```sh
> git checkout branch-with-my-new-awesome-icons
> git merge master
# ..lots of output including
Auto-merging icons/some/file.dmi
CONFLICT (content): Merge conflict in icons/some/file.dmi
# ..more output

# optionally
> git status
# ..some output including
Unmerged paths:
  (use "git add <file>..." to mark resolution)
        both modified:   icons/some/file.dmi

```
2) Run `dmi-merge-tool.exe` with path to your repository as an argumet.
```sh
> dmi-merge-tool.exe D:/work/my/repo/

# alternatively
> cd path/to/my/repo
> C:/whatever/path/to/dmi-merge-tool.exe .
```
3) Try to read through the output. It would tell you it failed to deconflict stuff for whatever reason.
4) *RESAVE* the now-deconflicted files in dreammaker. That is, open dreammaker, navigate to the file, open it, *double-check that the changes make sense*, press ctrl+S, close. This tool only creates images that are barely readable by the DM; resaving optimizes them and makes them actually usable.
5) continue resolving the conflicts as you would normally in git. That is `git add` followed by `git commit`. Or whatever the equivalent for those is in your git tool of choice.
6) Done!

# License
MIT-0
