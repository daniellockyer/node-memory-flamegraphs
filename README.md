# 📊 node-memory-flamegraphs

Tool for generating memory flamegraphs of Node applications

## Usage

Once you have `node-memory-flamegraphs` installed, you can use it in a few different ways.

If you have a Node application already running with `node --inspect ...` or `node --inspect-brk ...`:

```bash
node-memory-flamegraphs
```

To supply the initial Node script to run in a debugger in the background, use `--entry-point`:

```bash
node-memory-flamegraphs --entry-point ./project/awesome.js
```

To delay collecting the samples, use `--delay`:

```bash
node-memory-flamegraphs --delay 5000
```

To change the sampling frequency, use `--frequency`:

```bash
node-memory-flamegraphs --frequency 500
```

Press Ctrl+C to finish sampling and to generate the flamegraph.

## Screenshot

![Example](https://user-images.githubusercontent.com/964245/158569136-432f0235-aec1-49b6-ac5a-cefe0b40bd20.svg)

## License

Copyright (c) 2022 Daniel Lockyer - Released under the [MIT license](LICENSE).
