# Project's post-mortem

_Development has been finished on 24.06.2024_, however, functionally the app was
at this stage a few weeks earlier. From now on contribution will depend on if
there's any interest at all in using the tool by the community and on feedback
from users. I don't really have plans to implement any new features, but I'm
open to PRs and suggestions. I will, however, try to keep the dependencies
up-to-date with the help of dependabot and fix any reported bugs.

## Initial plan for the project

- The app was meant to aggregate ratings from several websites: filmweb.pl,
  imdb.com and rottentomatoes.com but since these sites either don't have open
  APIs or access to them is paid, I decided to use TMDB API. Which is freely
  available and doesn't require website scraping which in itself adds complexity
  with modern JS-filled websites. IMO this is not a big deal since the user
  still gets the ratings and summaries which is what I cared about in the first
  place. The decision was also dictated by the fact that there were already
  enough columns to fill the full screen terminal and adding more ratings would
  in my opinion deteriorate the "UI".
- The app was meant to scrape repertoires
  and venues for 3 major polish cinema chains: Cinema city, Helios and
  Multikino, however, apart from CC the other sites were problematic to scrape
  even though I used the same chromium-driven library to fetch and render JS
  content as I did for CC. I think to overcome this problem I should probably
  switch over to Selenium or create a node.js script to render the full websites
  contents as one can see it in their browser but at that point I admit I was
  tired of this project knowing full well that it most likely will be only used
  by its author alone :)
- Last but not least, I intended to bundle the app into a single executable file
  using PyInstaller, but it was problematic and again - I didn't want to spend
  more time on this project than I already have, so I skipped this step.

## Expected outcomes

:white_check_mark: Have a simple functional CLI tool to quickly look up the
repertoire & ratings to sift out crap when looking for a movie to watch in
cinema.

:white_check_mark: Have a complete, a bit more complex scraping project in my
portfolio.

:x: Package the app into a single executable file (and learn the process along
the way).

:x: Have a tool to quickly look up screening times across multiple venues and
cinema chains (only completed for Cinema City).

:white_check_mark: Implement CICD pipeline with GitHub actions to run linter and
tests (and build + deploy but that went out of the window with PyInstaller).
