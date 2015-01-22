=======================
Container Configuration
=======================

Container configuration is a YAML file which is usually put into
``/config/<service_name>.yaml`` into container image itself.

.. note:: Curently container configuration may be put into any folder inside
   the image, but we may fix this folder later. The arbitrary path for
   container configuration may be a security vulnerability.

The somewhat minimal configuration is looks like following:

.. code-block:: yaml

    kind: Daemon
    user_id: 1
    volumes:
      /tmp: tmpfs:size=100m
    executable: /bin/sleep
    arguments: [60]


Reference
=========

``kind``
    One of ``Daemon`` (default) and ``Command``.

    The ``Daemon`` is long-running process that is monitored by supervisor.

    The ``Command`` things are just one-off tasks, for example to initialize
    local file system data, or to check health of daemon process. The
    ``Command`` things are run by ``lithos_cmd`` utility

``user-id``
    The numeric user indentifier for the process. It must be one of the allowed
    values in lithos configuration. Usually value of ``0`` is not allowed.

``group-id``
    The numeric group indentifier for the process. It must be one of the
    allowed values in lithos configuration. Usually value of ``0`` is not
    allowed.

``memory-limit``
    The memory limit for process and it's children. This is enforced by
    cgroups. Default: nolimit. (Doesn't work yet)

``fileno-limit``
    The limit on file descriptors for process. Default ``1024``.

``restart-timeout``
    The minimum time to wait between subsequent restarts of failed processes
    in seconds.  This is to ensure that it doesn't boggles down CPU. Default
    is     ``1`` second. It's enough so that lithos itself do not hang. But
    it should be bigger for heavy-weight processes. Note: this is time between
    restarts, i.e. if process were running more than this number of seconds
    it will be restarted immediately.

``executable``
    The path to executable to run. Only absolute paths are allowed.

``arguments``
    The list of arguments for the command. Except argument zero.

``environ``
    The mapping of values that are set for process. You must set all needed
    environment variables here. The only variable that is propagated by
    default is ``TERM``. Also few special ``LITHOS_`` variables may be set.
    This means you must set all the basic ``LANG``, ``HOME`` and so on
    explicitly. This is to ensure that your environment is always the same
    regardless of where you run process.

``workdir``
    The working directory for target process. Default is ``/``. Working
    directory must be absolute.

``resolv-conf``
    Parameters of the ``/etc/resolv.conf`` file to generate. Default
    configuration is::

        resolv-conf:
            copy-from-host: true

    Which means ``resolv.conf`` from host where lithos is running is copied
    to the "state" directory of the container. More options are expected to
    be added later.

    .. warning:: To make use of it you should symlink ``ln -s
       /state/resolv.conf /etc/resolv.conf`` in the container's image. It's
       done this way so you can introspect and presumably update
       ``resolv.conf`` from the outside of container.

``hosts-file``
    Parameters of the ``/etc/hosts`` file to generate. Default
    configuration is::

        hosts-file:
            localhost: true
            public-hostname: true

    Which means add to ``hosts`` file the ``localhost`` name and the result
    of ``gethostname`` system call along with the ip address that name
    resolves into.

    .. warning:: To make use of it you should symlink``ln -s
       /state/hosts /etc/hosts`` in the container's image. It's
       done this way so you can introspect and presumably update
       ``hosts`` from the outside of container.

``uid-map``, ``gid-map``
    The list of mapping for uids(gids) in the user namespace of the container.
    If they are not specified the user namespace is not used. This setting
    allows to run processes with ``uid`` zero without the risk of being
    the ``root`` on host system.

    Here is a example of maps::

        uid-map:
        - {inside: 0, outside: 1000, count: 1}
        - {inside: 1, outside: 1, count: 1}
        gid-map:
        - {inside: 0, outside: 100, count: 1}

``stdout-stderr-file``
    This redirects both stdout and stderr to a file. The path must be absolute
    and is opened outside of the container (so depending on ``volumes`` may be
    both visible or non-visible from the container itself)

``volumes``
    The mapping of mountpoint to volume definition. See volumes_ for more
    info



