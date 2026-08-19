# Codex Cost Meter

The utility reports Codex usage from local storage. Project statistics group thread reports by the desktop project's membership while retaining CLI-only work.

## Project statistics

**Desktop Project**:
A named Codex desktop-app project with one or more source roots.
_Avoid_: Folder, repository

**Source Root**:
A directory associated with a Desktop Project as a source location.
_Avoid_: Current working directory, worktree

**Project Assignment**:
The desktop app's recorded association between a thread and a Desktop Project.
_Avoid_: Current working directory, workspace match

**Project Ref**:
Human CLI input that resolves to a Desktop Project, a local path, or a historical starting directory.
_Avoid_: Project ID, target

**Projectless Thread**:
A desktop-app thread explicitly recorded outside every Desktop Project.
_Avoid_: Unassigned thread

**Workspace Fallback**:
An unassigned CLI root thread included because its recorded starting directory is equal to or beneath the selected project's source roots. Later directory changes never affect it.
_Avoid_: Project Assignment

**Project Report**:
A lifetime aggregate of existing root-thread reports selected through a Desktop Project, path, or thread.
_Avoid_: Directory report, project statistics
