# GitHub sign-in

Each profile can be signed in to a GitHub account from the profile menu (the profile button in the activity bar).

## For users

1. Open the profile menu and choose **Sign in with GitHub**.
2. Graphite opens github.com in your browser and shows a short code in the status bar, for example `WDJB-MJHT`.
3. Enter the code on the GitHub page and approve Graphite. Graphite waits for the approval and then shows **Signed in as @your-login**.

The menu then shows your account and **Sign out of GitHub**. Signing out removes the saved sign-in from this computer; it does not revoke the approval on GitHub. To revoke it, open your GitHub settings under *Applications → Authorized OAuth Apps*.

Graphite asks GitHub only for `read:user`, which gives the public profile (login and name). It does not read repositories or private data.

### Where the sign-in is kept

The access token is saved as `github-session.json` in the profile folder. On Linux and macOS the file is readable only by your user account. On Windows it is protected by the permissions of your user profile folder. Each profile has its own sign-in.

## For people who distribute Graphite

Graphite ships with its own GitHub OAuth App, so users need no setup. The App is registered with the **Device Flow** enabled, and its public client ID (`Ov23lieI1U1RTbC3aaHz`) is compiled into every build. The device flow never uses a client secret, so no secret is stored in the code.

To use a different OAuth App (for example, for a fork), create one at *GitHub → Settings → Developer settings → OAuth Apps*, enable **Enable Device Flow**, and build with its client ID:

```
GRAPHITE_GITHUB_CLIENT_ID=<your client id> cargo build --release -p graphite-app
```

The environment variable replaces the built-in ID for that build. Never add a client secret to Graphite.

## Limits

- The sign-in needs internet access to github.com and api.github.com.
- A code expires after about 15 minutes. If it expires, choose sign-in again.
- Extensions can use the account through `authentication.getSession("github", ...)`, but only after you allow each one in a prompt. The session remembers the scopes it was issued with; an extension that needs more scopes starts a new sign-in. See [Extension API](extension-api.md).
