# Run menu

The Run menu exposes Graphite's DAP controls and line-breakpoint actions. Start Debugging requires a trusted workspace and a valid `.vscode/launch.json`. **Add Configuration** creates a stdio-adapter starter profile when the file is missing; **Open Configurations** opens the file when it exists.

Stop, restart, continue and stepping commands are enabled according to the active debug session. Conditional and log breakpoints accept their DAP fields in a small editor prompt. Breakpoints can be enabled, disabled or removed in bulk, and changes are sent to an active adapter.

Run Without Debugging, inline/function/triggered breakpoints and installing additional debuggers are shown disabled until Graphite supports those capabilities. Selecting a disabled action displays its reason in the status bar, and accessibility tools receive the same explanation. The menu is navigable with the pointer and keyboard. Labels and prompts are available in English and Brazilian Portuguese.
