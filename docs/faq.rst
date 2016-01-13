==========================
Frequently Asked Questions
==========================


How do I Start/Stop/Restart Processes Running By Lithos?
========================================================

Short answer: You can't.

Long answer: Lithos keep running all the processes that it's configured to
run. So:

* To stop process: remove it from the config
* To start process: add it to the config. If it's added, it will be restarted
  indefinitely. Sometimes may want to fix :opt:`restart-timeout`
* To restart process: well, kill it (with whatever signal you want).

The ergonomic of these operations is intentionally not very pleasing. This is
because you are supposed to have higher-level tool to manage lithos. At least
you want to use ansible_, chef_ or puppet_.

.. _ansible: http://ansible.com/
.. _chef: http://chef.io/
.. _puppet: http://puppetlabs.com/


Why /run/lithos/mnt is empty?
=============================

This is a mount point. It's never mounted in host system namespace (well it's
never visible in guest namespace too). The containerization works as follows:

1. The mount namespace is *unshared* (which means no future mounts are visible
   in the host system)
2. The root filesystem image is mounted to ``/run/lithos/mnt``
3. Other things set up in root file system (``/dev``, ``/etc/hosts``, whatever)
4. Pivot root is done, which means that ``/run/lithos/mnt`` is now visible as
   root dir, i.e. just plain ``/`` (you can think of it as good old ``chroot``)

This all means that if you error like this::

    [2015-11-17T10:29:40Z][ERROR] Fatal error: Can't mount pseudofs /run/lithos/mnt/dev/pts (newinstance, options: devpts): No such file or directory (os error 2)

Or like this::

    [2015-10-19T15:04:48Z][ERROR] Fatal error: Can't mount bind /whereever/external/storage/is to /run/lithos/mnt/storage: No such file or directory (os error 2)

It means that lithos have failed on step #3. And that it failed to mount the
directory in the guest container file system (``/dev/pts`` and ``/storage``
respectively)


How to Organize Logging?
========================

There is variety of ways. Here are some hints...


Syslog
------

You may accept logs by UDP. Since lithos has no network namespacing (yet).
The UDP syslog just works.

To setup unix soket syslog you may configure syslog daemon on the
host system to listen for the socket inside the container's ``/dev``.
For example, here is how to `configure rsyslog`__ for default lithos config::

    module(load="imuxsock") # needs to be done just once
    input(type="imuxsock" Socket="/var/lib/lithos/dev/log")

__ http://www.rsyslog.com/doc/v8-stable/configuration/modules/imuxsock.html

Alternatively, (but *not* recommended) you may configure :opt:`devfs-dir`::

    devfs-dir: /dev


Stdout/Stderr
-------------

It's recommended to use syslog or any similar solutions for logs. But there
are still reasons to write logs to a file:

1. You may want to log early start errors (when you have not yet initialized
   the logging subsystem of the application)
2. If you have single server and don't want additional daemons

Lithos has strong preference to make anything configuration from container
config if possible. This kind of logging is no exception. So you can
do logging with config similar to:

.. code-block:: yaml

    volumes:
      /log: !Persistent { path: "/log" }

    stdout-stderr-file: /log/stdio.log

See :ref:`container_config` for more info. You also need ``/log`` to be in
:opt:`writable-paths`.

What If I don't Configure Stdout/Stderr?
----------------------------------------

The stdout and stderr are just inherited from ``lithos_tree``. So for example
if you had started lithos by ``upstart`` your logs will end up in the
``/var/log/upstart/lithos.log`` by default. All sandboxes will be mixed
together.


Why There is no per-Sandbox Logging?
------------------------------------

Because another useful thing in lithos is ``lithos_cmd`` which runs from
the shell that started the command. This means that both writing the log
to the default place and putting it to the user screen will be unintuitive
to do depending on your current point of view.

So we prefer user to explicitly state where to log data to in each command and
always inherit stdout/stderr by default.
