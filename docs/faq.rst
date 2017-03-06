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

To setup syslog using unix sockets you may configure syslog daemon on the
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

Starting with version ``v0.5.0`` lithos has a per-sandbox log file which
contains all the stdout/stderr output of the processes. By default it's in
``/var/log/lithos/stderr/<sandbox_name>.log``. See :opt:`stdio-log-dir` for
more info.


How to Update Configs?
======================

The best way to update config of *processes* is to put it into a temporary
file and run ``lithos_switch`` (see ``lithos_switch --help`` for more info).
This is a main kind config you update multiple times a day.

In case you've already put config in place, or for *master* and *sandbox*
config, you should first run ``lithos_check`` to check that all configs are
valid.  Then just send ``QUIT`` signal to the ``lithos_tree`` process. Usually
the following command-line is enough for manual operation::

    pkill -QUIT lithos_tree

But if you for authomation it's better to use ``lithos_switch``.

.. note:: note

   By sending ``QUIT`` signal we're effectivaly emulate crash of the supervisor
   daemon. It's designed in a way that allows it survive crash and keep all
   fresh child processes alive. After an **in-place restart** it checks
   configuration of child processes, kills outdated ones and executes new
   configs.


.. _running-commands:

How to Run Commands in Container?
=================================

There are two common ways:

1. If you have container already running use ``nsenter``
2. Prepare a special command for ``lithos_cmd``


Running ``nsenter``
-------------------

This way only works if you have a running container. It's hard to get work if
your process crashes too fast after start.

You must also have a working shell in container, we use ``/bin/sh``
in examples.

You can use ``nsenter`` to join most namespaces, except user namespace.
For example, if you know pid, the following command would allow you to run
shell in container and investigate files::

    nsenter -m -p --target 12345 /bin/sh

If you don't know PID, you may easily discover it with ``lithos_ps`` or
automate it with ``pgrep``::

    nsenter -m -p \
        --target=$(pgrep -f 'lithos_knot --name sandbox-name/process-name.0') \
        /bin/sh

.. warning:: This method is very insecure. It runs command in original user
   namespace with the host root user. While basic sandboxing (i.e. filesystem
   root) is enabled by `-m` and `-p`, the program that you're trying to
   run (i.e. the shell itself) can still escape that sandbox.

   Because we do mount namespaces and user namespaces in different stages of
   container initialization there is currently no way to join both
   user namespace and mount namespace. (You can join just user namespace
   by running ``nsenter -U --target=1235`` where 123 is the pid of the
   process inside the container, not lithos_knot. But this is probably useless)


Running ``lithos_cmd``
-----------------------

In some cases you may want to have a special container with a shell to run
with ``lithos_cmd``. This is just a normal lithos container configuration
with ``kind: Command`` and ``interactive: true`` and shell being specified
as a command. So you run your ``shell.yaml`` with::

    lithos_cmd sandbox-name shell

There are three important points about this method:

1. If you're trying to investigate problem with the daemon config you copy
   daemon config into this interactive command. It's your job to keep both
   configs in sync. This config must also be exposed in *processes* config
   just like any other.

2. It will run another (although identical) container on each run. You will
   not see processes running as daemons and other shells in ``ps`` or similar
   commands.

3. You must have shell in container to get use of it. Sometimes you just don't
   have it. But you may use any interactive interpreter, like ``python`` or
   even non-interactive commands.


.. _find-files:

How to Find Files Mounted in Container?
=======================================

Linux provides many great tools to introspect running container. Here
is short overview:

1. ``/proc/<pid>/root`` is a directory where you can ``cd`` into and look
   at files
2. ``/proc/<pid>/mountinfo`` is a mapping between host system directories
   and ones container
3. And you can :ref:`join container's namespace <running-commands>`


Example 1
---------

Let's try to explore some common tasks. First, let's find container's pid::

    $ pgrep -f 'lithos_name --name sandbox-name/process-name.0'
    12345

Now we can find out the OS release used to build container::

    $ sudo cat /proc/12345/root/etc/alpine-release
    3.4.6

.. warning:: There is a caveat. Symlinks that point to paths starting with
   root are resolved differently that in container. So ensure that you're
   not accessing a symlink (and that any intermediate components is not
   a symlink).


Example 2
---------

Now, let's find out which volume is mounted as ``/app/data`` inside the
container.

If you have quire recent ``findmnt`` it's easy::

    $ findmnt -N 12345 /app/data
    TARGET     SOURCE                                         FSTYPE OPTIONS
    /app/data  /dev/mapper/Disk-main[/all-storages/myproject] ext4   rw,noatime,discard,data=ordered

Here we can see that ``/app/data`` in container is a LVM partition ``main``
in group ``Disk`` with the path ``all-storages/myproject`` relative to
the root of the partition. You can find out where this volume is mounted on
host system by inspecting the output of ``mount`` or ``findmnt`` commands.

Manual way is to look at ``/proc/<pid>/mountinfo`` (stripped output)::


    $ cat /proc/12345/mountinfo
    347 107 9:1 /all-images/sandbox-name/myproject.c17cb162 / ro,relatime - ext4 /dev/md1 rw,data=ordered
    356 347 0:267 / /tmp rw,nosuid,nodev,relatime - tmpfs tmpfs rw,size=102400k
    360 347 9:1 /all-storages/myproject /app/data rw,relatime - ext4 /dev/mapper/Disk-main rw,data=ordered

Here you can observe same info. Important parts are:

* Fifth column is the mountpoint (but be careful in complex cases there might
  be multiple overlapping mount points);
* Fourth column is the path relative to the volume root;
* And, 9th column (next to the last) is the volume name.

Let's find out where it is on host system::

    $ mount | grep Disk-main
    /dev/mapper/Disk-main on /srv type ext4 (rw,noatime,discard,data=ordered)

That's it, now you can look at ``/srv/all-storages/myproject`` to find files
seen by an application.
