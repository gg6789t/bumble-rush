# bumble rush

a simple game to kill time

it's inspired by flappy bird, but with a bee, power-ups, and a sky that changes as you go.

it runs right in your terminal and uses colored half-block characters so it actually looks like pixel art instead of ascii soup. the sky slowly goes from day to sunset to night the further you get, which is honestly my favorite part.

![screenshot](assets/screenshot.png)

## install

with homebrew (mac or linux):

```sh
brew install gg6789t/tap/bumble-rush
```

with cargo, if you have rust:

```sh
cargo install --git https://github.com/gg6789t/bumble-rush
```

then just run `bumble-rush`.

an AUR package is ready too, it'll go up once AUR opens new accounts again.

## how to play

space, w, up arrow or clicking makes the bee flap. p pauses, q or esc quits. that's it.

some stuff that shows up while you play:

- honey drops are worth a point each
- power-ups: **S** is a shield that saves you from one crash (even the ground), **T** slows everything down for a few seconds, **M** pulls honey towards you, **2** gives double points
- if you barely squeeze past a pillar you get a bonus point for the close call
- after a while purple crystal pillars start showing up and those ones move, so watch out
- you get a medal at 10, 25, 50 and 100. good luck with diamond

your best score is saved in `~/.bumble_rush_best`.

## if the colors look weird

it tries to figure out if your terminal supports full 24-bit color. if it guesses wrong you can force it with `--truecolor` or `--256`. your terminal needs to be at least 40x16.

## license

MIT
