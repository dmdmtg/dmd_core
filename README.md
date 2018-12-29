# AT&T DMD5620 Core

[![Build Status](https://travis-ci.org/sethm/dmd_core.svg?branch=master)](https://travis-ci.org/sethm/dmd_core)

Core logic for an AT&T / Teletype DMD 5620 terminal emulator

## Description

The AT&T / Teletype DMD 5620 terminal was a portrait display,
programmable, windowing terminal produced in the early 1980s. It came
out of research pioneered by Rob Pike and Bart Locanthi Jr., of AT&T
Bell Labs.

![DMD 5620 Terminal](https://static.loomcom.com/3b2/5620/dmd5620.jpg)

This project implements the core logic needed to emulate a DMD 5620
terminal, including:

- ROM
- RAM
- WE32100 CPU
- I/O

Note that there is no user interface: This is a back-end library only.
It may be used as a component to build a fully-fledged emulator,
however.

## Changelog

0.6.1: Fix failing tets.

0.6.0: Breaking change. Refactor DMD a bit to rename the function
       `read` to `read_word`, and added a `read_byte` function
       as well. Also refactored the CPU mnemonic lookup to be
       more efficient using a lookup table instead of a HashMap.
       Lastly, fixed a bug in the DUART that set the wrong
       delay for one baud rate.

0.5.0: Non-breaking but major change. `dmd_core` now presents a
       C compatible API to make interacting with C and C++ code
       easier, without needed to write a full stub library.

0.4.0: Breaking change. TX to the keyboard from the terminal is
       now supported, so that clients can use it to detect when
       a bell (^G) request has been sent.

0.3.1: Added exception handling for memory errors, and a `run` function
       to free-run the CPU for a given number of steps.

0.3.0: Breaking change. Charater RX from RS-232 and keyboard are now
       handled by internal queues, and no longer return `DuartError`
       on failure.

0.2.1: Initial release.

## Emulator Reference Implementations

Two implementations of DMD 5620 emulator use this core library.

* The Linux native GTK application `dmd5620`: [https://github.com/sethm/dmd_gtk](https://github.com/sethm/dmd_gtk)

* The Macintosh native Cocoa application `DMD 5620`: [https://github.com/sethm/dmd_mac](https://github.com/sethm/dmd_mac)

A Windows native application is in the works.
