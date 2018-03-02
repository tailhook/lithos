# Lithos

[Documentation](http://lithos.readthedocs.org)


Lithos is a process supervisor and containerizer for running services. Lithos
is not intended to be system init. But rather tries to be a base tool to build
container orchestration.

Features:

* use linux namespaces and cgroups for containerization
* immediate restart of failing processes (with rate limit of course)
* in-place upgrade of lithos without touching child processes
* written in Rust so memory-safe and has zero runtime dependencies

It's designed to have absolutely minimal required functionality. In particular
it doesn't include:

* an image downloader (``rsync`` is super-cool) or builder (use any tool)
* any network API


## Running Examples

Testing it in vagrant::

    vagrant up && vagrant ssh

In vagrant shell::

    $ ./example_configs.sh
    $ sudo lithos_tree

If you want to change containers, sources or configs of this test vagrant
deployment just rerun ``./example_configs.sh``.

(Note: in this test deployment lithos doesn't properly reload configs, because
images does not version properly. Just restart `lithos_tree` to apply the
changes)


License
=======

Licensed under either of

* Apache License, Version 2.0,
  (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)
  at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

