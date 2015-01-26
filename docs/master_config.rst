====================
Master Configuration
====================


Master configuration file is the one that usually at ``/etc/lithos.yaml`` and
defines small subset of global configuration parameters. Minimal configuration
is an *empty file* but it **must exist** anyway. Here is the reference of
the parameters along with the default values:

``config-dir``
    The directory for per-application configuration files. Path should be
    absolute.  Default is ``/etc/lithos``.

``runtime-dir``
    The directory where ``pid`` file of master process is stored and also
    the base directory for ``state-dir`` and ``mount-dir``. Path must be
    absolute. It's expected to be stored on ``tmpfs``. Default
    ``/run/lithos``.

``state-dir``
    The directory where to keep container's state dirs. If path is relative
    it's relative to ``runtime-dir``. Default ``state``
    (i.e. ``/run/lithos/state``). Path should be on ``tmpfs``.

``mount-dir``
    An empty directory to use for mounting. If path is relative it's relative
    to ``runtime-dir``. Default ``mnt``.

``devfs-dir``
    The directory where ``/dev`` filesystem for container exists. If it's
    not ``/dev`` (which is not recommended), you should create the directory
    with ``lithos_mkdev`` script. Default ``/var/lib/lithos/dev``.

``cgroup-name``
    The name of the root cgroup for all lithos processes. Specify ``null`` (or
    any other form of YAMLy null) to turn cgroups off completely.

``cgroup-controllers``
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
