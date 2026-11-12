# Privacy policy

GlideDeck is a desktop app that works offline. It has no accounts, no analytics and no
telemetry, and it never sends your presentations anywhere.

This program will not transfer any information to other networked systems unless
specifically requested by the user or the person installing or operating it.

## When GlideDeck goes online

Only when you ask it to:

- **Check for updates** (About window, or File → Check for updates): GlideDeck asks GitHub
  for the latest release of GlideDeck, and if you choose to download it, downloads the
  installer from GitHub. See [GitHub's privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
- **Web videos** you add to a slide (YouTube, Vimeo, Loom, or a link to a video file):
  when you add one, GlideDeck fetches its title and poster image from that site, and when
  you play one, it streams the video from that site. If you installed yt-dlp, GlideDeck
  runs it to find the video's stream. Those sites' own policies apply:
  [YouTube / Google](https://policies.google.com/privacy),
  [Vimeo](https://vimeo.com/privacy), [Loom](https://www.atlassian.com/legal/privacy-policy).

## Web pages you export

A presentation exported as a web page (Present → Web page) is a file you control. If it
contains web videos, the page loads the site's own video player when a viewer plays one:
YouTube's privacy-enhanced player (youtube-nocookie.com), Vimeo's player with
"do not track" set, or Loom's player. Those players are covered by the policies above.

## The AI assistant server (MCP)

`glidedeck mcp` runs on your computer and talks only to the AI app that started it. It goes
online only for the same reasons as above, when the assistant asks for them (for example,
adding a web video).

## Files on your computer

GlideDeck keeps a few files in your settings folder (`%APPDATA%\GlideDeck` on Windows,
`~/Library/Application Support/GlideDeck` on macOS): the recent files list, your AutoSave
preference, and recovery copies of unsaved work, which are deleted when you save or close
normally.
