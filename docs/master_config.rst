.. _master_config:

====================
Master Configuration
====================


Master configuration file is the one that usually at
``/etc/lithos/master.yaml`` and defines small subset of global configuration
parameters. Minimal configuration is an *empty file* but it **must exist**
anyway. Here is the reference of the parameters along with the default values:

.. opt:: sandboxes-dir

    The directory for per-application configuration files which contain limits
    of what application might use. If path is relative it's relative to
    the directory where configuration file is. Default is ``./sandboxes``.

.. opt:: processes-dir

    The directory for per-application configuration files which contain name of
    image directory, instance number, etc., to run. If path is relative it's
    relative to the directory where configuration file is. Default is
    ``./processes``.

.. opt:: runtime-dir

    The directory where ``pid`` file of master process is stored and also
    the base directory for ``state-dir`` and ``mount-dir``. Path must be
    absolute. It's expected to be stored on ``tmpfs``. Default
    ``/run/lithos``.

.. opt:: state-dir

    The directory where to keep container's state dirs. If path is relative
    it's relative to ``runtime-dir``. Default ``state``
    (i.e. ``/run/lithos/state``). Path should be on ``tmpfs``.

.. opt:: mount-dir

    An empty directory to use for mounting. If path is relative it's relative
    to ``runtime-dir``. Default ``mnt``.

.. opt:: devfs-dir

    The directory where ``/dev`` filesystem for container exists. If it's
    not ``/dev`` (which is not recommended), you should create the directory
    with ``lithos_mkdev`` script. Default ``/var/lib/lithos/dev``.

.. opt:: cgroup-name

    The name of the root cgroup for all lithos processes. Specify ``null`` (or
    any other form of YAMLy null) to turn cgroups off completely.

.. opt:: cgroup-controllers

    List of cgroup controllers to initialize for each container. Note: the
    empty list is treated as default. Default is
    ``[name, cpu, cpuacct, memory, blkio]``. If you have some controllers
    joined together like ``cpu,cpuacct`` it's ok.

    Use ``cgroup-name: null`` to turn cgroup tracking off (not empty list
    here).  And use ``cgroup-controllers: [name]`` to only use cgroups for
    naming processes but not for resource control.

    .. note:: turning off cgroups means that resource limits does not work
       completely. lithos will not try to enforce them by polling or some
       other means

.. opt:: default-log-dir

   (default ``/var/log/lithos``) The directory where master and each of the
   application logs are created (unless are overrided by sandbox config).

.. opt:: log-file

   (default ``master.log``) Master log file. Relative paths are treated from
   :opt:`default-log-dir`.

.. opt:: log-level

   (default ``warn``) Level of logging. Can be overriden on the command line.

.. opt:: syslog-facility

   (no default) Enables logging to syslog (with specified facility) instead of
   file.

.. opt:: syslog-name

   (default ``lithos``) Application name for master process in syslog. The
   child processes are prefixed by this value. For example ``lithos-django``
   (where ``django`` is a sandbox name).
