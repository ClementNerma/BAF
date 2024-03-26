# Basic Archive Format

BAF, short for **Basic Archive Format**, is a dead-simple file archive format.

You may see it as an alternative to the Tape Archive Format (TAR) but designed for different use cases.

The main advantages of BAF are:

* Archives are tailored for random-access (e.g. seeking) ;
* Files can be added or removed without rebuilding the whole archive ;
* Renamings don't need any rebuild either

The format is also designed to be extremely simple and almost anybody can make an encoder / decoder for it. You can see the whole specifications in the [`docs/specs.md`](docs/specs.md) document.
