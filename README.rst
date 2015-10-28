======
Lithos
======

:Status: beta
:Documentation: http://lithos.readthedocs.org


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


Running Examples
================

There is a configuration for building test image for vagga. You may use another
tool to build image. For vagga just run::

    vagga _build busybox

If you haven't built image by vagga you need to build it yourself and change
``image-dir`` setting in ``examples/sleep/limits/sleep.yaml``.

You also need to create ``/dev`` folder if you haven't run lithos before::

    lithos_mkdev /var/lib/lithos/dev

Now, run lithos, but specify full path to the config::

    sudo lithos_tree --config ${PWD}/examples/sleep/master.yaml

.. warning:: Lithos will clobber runtime directories ``/run/lithos``,
    so don't run examples on production machine. You can also change the
    runtime and log directories in configuration if you wish.

