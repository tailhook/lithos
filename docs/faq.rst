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
