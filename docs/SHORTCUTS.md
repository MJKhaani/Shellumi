# Keyboard Shortcuts Reference

This document provides a comprehensive reference of all keyboard shortcuts available in Shellumi across macOS, Linux, and Windows.

Shortcuts are designed to enable rapid navigation, seamless session management, and keyboard-centric terminal workflows without requiring mouse interactions.

---

## 1. Global & Application Shortcuts

These shortcuts operate globally across all views and pages of the application.

| Action | macOS | Linux / Windows | Description |
| :--- | :--- | :--- | :--- |
| **Command Palette** | `Cmd + K` | `Ctrl + K` | Open the unified command palette to search hosts, run snippets, switch views, or execute actions. |
| **Quick Connect** | `Cmd + O` | `Ctrl + O` | Open the quick connect input bar to connect to any host (`user@hostname:port`). |
| **New Host** | `Cmd + N` | `Ctrl + N` | Open the host editor modal to add a new server connection to local storage. |
| **Refresh Inventory** | `Cmd + R` | `Ctrl + R` | Reload host inventory, groups, and active tunnel states from the local database. |
| **Settings / Preferences** | `Cmd + ,` | `Ctrl + ,` | Open the preferences and application configuration view. |
| **Toggle Sidebar** | `Cmd + \` | `Ctrl + \` | Expand or collapse the navigation sidebar to maximize workspace area. |

---

## 2. Tabs & Session Management

Manage open connections, local shell sessions, and navigation between active tabs.

| Action | macOS | Linux / Windows | Description |
| :--- | :--- | :--- | :--- |
| **New Tab / Connection** | `Cmd + T` | `Ctrl + T` | Open a new session tab or return to the connection inventory. |
| **Open Local Terminal** | `Cmd + L` | `Ctrl + L` | Instantly launch a new local shell terminal session (`/bin/zsh`, `/bin/bash`, or `PowerShell`). |
| **Close Current Tab** | `Cmd + W` | `Ctrl + W` | Terminate and close the currently active session tab. |
| **Next Tab** | `Cmd + Shift + ]` / `Ctrl + Tab` | `Ctrl + Tab` / `Ctrl + PageDown` | Cycle forward to the next open session tab. |
| **Previous Tab** | `Cmd + Shift + [` / `Ctrl + Shift + Tab` | `Ctrl + Shift + Tab` / `Ctrl + PageUp` | Cycle backward to the previous open session tab. |
| **Switch to Tab 1–9** | `Cmd + 1` … `Cmd + 9` | `Ctrl + 1` … `Ctrl + 9` | Jump directly to the tab at position 1 through 9. |
| **Jump Between Tabs** | `Cmd + J` | `Ctrl + J` | Open the tab switcher to filter and jump between active sessions. |
| **Reconnect Session** | `Cmd + Shift + R` | `Ctrl + Shift + R` | Re-establish the connection for the active session without closing the tab. |

---

## 3. Split Views & Multi-Pane Layouts

Organize multiple terminal sessions side-by-side or stacked in a single window tab.

| Action | macOS | Linux / Windows | Description |
| :--- | :--- | :--- | :--- |
| **Split Vertically** | `Cmd + D` | `Ctrl + Shift + E` | Split the current terminal pane into two vertical columns side-by-side. |
| **Split Horizontally** | `Cmd + Shift + D` | `Ctrl + Shift + O` | Split the current terminal pane into two horizontal rows stacked vertically. |
| **Focus Next Pane** | `Cmd + ]` | `Alt + Right` | Move keyboard focus to the next split terminal pane in the active tab. |
| **Focus Previous Pane** | `Cmd + [` | `Alt + Left` | Move keyboard focus to the previous split terminal pane in the active tab. |
| **Close Active Pane** | `Cmd + Shift + W` | `Ctrl + Shift + W` | Close the currently focused split pane without closing the entire tab. |
| **Switch Focus Mode / Split Toggle** | `Cmd + Shift + M` | `Ctrl + Alt + M` | Toggle between split layout and single maximized pane focus. |
| **Toggle Broadcast Input** | `Cmd + B` | `Ctrl + Alt + B` | Enable or disable broadcasting keystrokes synchronously to all visible panes. |

---

## 4. Terminal Operations & Utilities

Actions available while focused in a terminal session.

| Action | macOS | Linux / Windows | Description |
| :--- | :--- | :--- | :--- |
| **Find in Terminal** | `Cmd + F` | `Ctrl + F` | Search the scrollback buffer and active screen for text patterns. |
| **Toggle Snippets Panel** | `Cmd + .` | `Ctrl + .` | Open or hide the side panel containing saved command snippets. |
| **Zoom In (Increase Font)** | `Cmd + =` / `Cmd + +` | `Ctrl + =` / `Ctrl + +` | Increase the terminal font size by 1 point. |
| **Zoom Out (Decrease Font)** | `Cmd + -` | `Ctrl + -` | Decrease the terminal font size by 1 point. |
| **Reset Zoom (Default Font)** | `Cmd + 0` | `Ctrl + 0` | Reset terminal font size to the user preference default (14px). |
| **Copy to Clipboard** | `Cmd + C` | `Ctrl + Shift + C` | Copy the selected terminal text to the operating system clipboard. |
| **Paste from Clipboard** | `Cmd + V` | `Ctrl + Shift + V` | Paste clipboard contents into the active terminal stream. |
| **Clear Terminal** | `Cmd + K` *(terminal)* | `Ctrl + Shift + K` | Clear screen output and reset scrollback view. |

---

## 5. Navigation Shortcuts

Jump directly to core infrastructure and tool views from anywhere in the application.

| View | macOS | Linux / Windows |
| :--- | :--- | :--- |
| **Hosts Inventory** | `Cmd + Shift + H` | `Ctrl + Shift + H` |
| **Overview Dashboard** | `Cmd + Shift + O` | `Ctrl + Shift + O` |
| **Workspaces** | `Cmd + Shift + K` | `Ctrl + Shift + K` |
| **SFTP File Manager** | `Cmd + Shift + U` | `Ctrl + Shift + U` |
| **Keychain Identities** | `Cmd + Shift + Y` | `Ctrl + Shift + Y` |
| **Port Forwarding Tunnels** | `Cmd + Shift + P` | `Ctrl + Shift + P` |
