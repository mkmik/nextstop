# Decisions / deviations from the PRD

One line per deviation, with the reason.

- Screenshots: `screencapture -l` of the real app is blocked on the dev machine (no Screen Recording permission for the shell), so milestone screenshots are rendered headlessly from the Vite dev server with Playwright/Chromium at 1×. Same DOM/CSS, same fonts; only the OS window frame is missing.
