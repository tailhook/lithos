.. highlight:: yaml

.. _container_config:

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
    user-id: 1
    volumes:
      /tmp: !Tmpfs { size: 100m }
    executable: /bin/sleep
    arguments: [60]

.. _container_variables:

Variables
=========

Container can declare some things, that can be changed in specific
instantiation of the service, for example:

.. code-block:: yaml

    variables:
      tcp_port: !TcpPort
    kind: Daemon
    user-id: 1
    volumes:
      /tmp: !Tmpfs { size: 100m }
    executable: /bin/some_program
    arguments:
    - "--listen=localhost:@{tcp_port}"

The ``variables`` key declares variable names and types. Value for these
variables can be provided in ``variables`` in :ref:`process_config`.

There are the following types of variables:

TcpPort
    Allows a number between 1-65535 and ensures that the number matches
    port range allowed in sandbox (see :opt:`allow-tcp-ports`)

    .. versionchanged:: 0.17.4

       Added ``activation`` parameter as a shortcut to support systemd
       activation protocol. I.e. the following (showing two ports
       for more comprehensive example):

       .. code-block:: yaml

          variables:
            port1: !TcpPort { activation: systemd }
            port2: !TcpPort { activation: systemd }

       Means to add something like this:

       .. code-block:: yaml

          variables:
            port1: !TcpPort
            port2: !TcpPort
          tcp-ports:
            "@{port1}":
              fd: 3
            "@{port2}":
              fd: 4
          environ:
            LISTEN_FDS: 1
            LISTEN_FDNAMES: "port1:port2"
            LISTEN_PID: "@{lithos:pid}"

        This works for any number of sockets. And it requires that
        ``LISTEN_FDS`, ``LISTEN_FDNAMES``, ``LISTEN_PID`` were absent in the
        ``environ`` as written in the file. Also it doesn't allow fine-grained
        control over parameters of the socket and file descriptor numbers.
        Use full form if you need specific options.

Choice
    Allows a value from a fixed set of choices
    (example: ``!Choice ["high-priority", "low-priority"]``)

Name
    Allows a value that matches regex ``^[0-9a-zA-Z_-]+$``. Useful for passing
    names of things into a script without having a chance to keep value
    unescaped when passing somewhere within a script or using it as a filename.

    .. versionadded:: 0.10.3

DottedName
    Allows arbitrary DNS-like name. It's defined as dot-separated name with
    only alphanumeric and underscores, where no component could start or end
    with a dash and no consequent dots allowed.

    .. versionadded:: 0.17.4

All entries of ``@{variable_name}`` are substituted in the following fields:

1. :opt:`arguments`
2. The values of :opt:`environ` (not in the keys yet)
3. The key in the :opt:`tcp-ports` (i.e. port number)

The expansion in any other place does not work yet, but may be implemented
in the future. Only **declared** variables can be substituted. Trying to
substitute undeclared variables or non-existing built-in variable results
into configuration syntax error.

There are the number of builtin variables that start with ``lithos:``:

lithos:name
    Name of the process, same as inserted in ``LITHOS_NAME`` environment
    variable

lithos:config_filename
    Full path of this configuration file as visible from within container

lithos:pid
    Pid of the process as visible inside of the container. Note: this variable
    can only be in environment and can only be full value of the variable.
    I.e. `PID: "@{lithos:pid}"` is fine,
    but `PID: "pid is @{lithos:pid}"` is **not allowed**. (In most cases
    this variable is exaclty ``2``, this is expected but might not be always
    true in some cases).


More built-in variables may be added in the future. Built-in variables
doesn't have to be declared.


Reference
=========

.. opt:: kind

    One of ``Daemon`` (default), ``Command`` or ``CommandOrDaemon``.

    The ``Daemon`` is long-running process that is monitored by supervisor.

    The ``Command`` things are just one-off tasks, for example to initialize
    local file system data, or to check health of daemon process. The
    ``Command`` things are run by ``lithos_cmd`` utility

    The ``CommandOrDaemon`` may be used in both ways, based on how it was
    declared in :ref:`Process Config <process_config>`. In the command
    itself you can distinguish how it is run by ``/cmd.`` in ``LITHOS_NAME``
    or cgroup name or better you can pass
    :ref:`variable <container_variables>` to a specific command and/or daemon.

    .. versionadded:: 0.10.3
       ``ContainerOrDaemon`` mode

.. opt:: user-id

    The numeric user indentifier for the process. It must be one of the allowed
    values in lithos configuration. Usually value of ``0`` is not allowed.

.. opt:: group-id

    The numeric group indentifier for the process. It must be one of the
    allowed values in lithos configuration. Usually value of ``0`` is not
    allowed.

.. opt:: memory-limit

    The memory limit for process and it's children. This is enforced by
    cgroups, so this needs `memory` cgroup to be enabled (otherwise its no-op).
    See :opt:`cgroup-controllers` for more info.  Default: nolimit.

    You can use ``ki``, ``Mi`` and ``Gi`` units for memory accounting.
    See integer-units_.

    .. versionchanged:: 0.14.0

       Previously it only set ``memory.limit_in_bytes`` but now it also sets
       ``memory.memsw.limit_in_bytes`` if the latter exists (otherwise skipping
       silently). This helps to kill processes earlier instead of swapping out
       to disk.

.. opt:: cpu-shares

    The number of CPU shares for the process. Default is ``1024`` which means
    all processes get equal share. You may split them to different values
    like ``768`` for one process and ``256`` for another one.

    This is enforced by cgroups, so this needs `cpu` cgroup to be enabled
    (otherwise its no-op).  See :opt:`cgroup-controllers` for more info.

.. opt:: fileno-limit

    The limit on file descriptors for process. Default ``1024``.

.. opt:: restart-timeout

    The minimum time to wait between subsequent restarts of failed processes
    in seconds.  This is to ensure that it doesn't boggles down CPU. Default
    is     ``1`` second. It's enough so that lithos itself do not hang. But
    it should be bigger for heavy-weight processes. Note: this is time between
    restarts, i.e. if process were running more than this number of seconds
    it will be restarted immediately.

.. opt:: kill-timeout

    (default ``5`` seconds) The time to wait for application to die. If it is
    not dead by this number of seconds we kill it with ``KILL``.

    You should not rely on this timeout to be precise for multiple reasons:

    1. Unidentified children are killed with a default timeout (5 sec).
       This includes children which are being killed when their configuration
       is removed.
    2. When lithos is restarted (i.e. to reload a configuration) during
       the timeout, the timeout is reset. I.e. the process may hang more than
       this time.

.. opt:: executable

    The path to executable to run. Only absolute paths are allowed.

.. opt:: arguments

    The list of arguments for the command. Except argument zero.

.. opt:: environ

    The mapping of values that are set for process. You must set all needed
    environment variables here. The only variable that is propagated by
    default is ``TERM``. Also few special ``LITHOS_`` variables may be set.
    This means you must set all the basic ``LANG``, ``HOME`` and so on
    explicitly. This is to ensure that your environment is always the same
    regardless of where you run process.

.. opt:: secret-environ

    Similarlty to ``environ`` but contains encrypted environment variables.
    For example::

        secret-environ:
          DB_PASSWORD: v2:ROit92I5:82HdsExJ:Gd3ocJsr:Hp3pngQZUos5b8ioKVUx40kegM1uDsYWwsWqC1cJ1/1KmQPQQWJZe86xgl1EOIxbuLj6PUlBH8yz5qCnWp//Ofbc

    Note: if environment variable is both in ``environ`` and ``secret-environ``
    which one overrides is not specified for now.

    You can encrypt variables using ``lithos_crypt``::

        lithos_crypt encrypt -k key.pub -d "secret" -n "some.namespace"

    You only need public key for encryption. So the idea is that public key
    is published somewhere and anyone, even users having to access to
    server/private key can add a secret.

    The ``-n`` / ``--namespace`` parameter must match one of
    the :opt:`secrets-namespaces` defined for project's sandbox.

    Usually there is only one private key for every deployment (cluster), and
    a single namespace per project. But in some cases you might need single
    lithos config for multiple destinations or just want to rotate private key
    smoothly. So you can put secret(s) encoded for multiple keys and/or
    namespaces:

    .. code-block:: yaml

        secret-environ:
          DB_PASSWORD:
          - v2:h+M9Ue9x:82HdsExJ:Gd3ocJsr:/+f4ezLfKIP/mp0xdF7H6gfdM7onHWwbGFQX+M1aB+PoCNQidKyz/1yEGrwxD+i+qBGwLVBIXRqIc5FJ6/hw26CE
          - v2:ROit92I5:cX9ciQzf:Gd3ocJsr:LMHBRtPFpMRRrljNnkaU6Y9JyVvEukRiDs4mitnTksNGSX5xU/zADWDwEOCOtYoelbJeyDdPhM7Q1mEOSwjeyO317Q==
          - v2:h+M9Ue9x:82HdsExJ:Gd3ocJsr:/+f4ezLfKIP/mp0xdF7H6gfdM7onHWwbGFQX+M1aB+PoCNQidKyz/1yEGrwxD+i+qBGwLVBIXRqIc5FJ6/hw26CE

    Note: technically you can encrypt different secrets here, we can't enforce
    that, but it's very discouraged.

    The underlying encyrption is curve25519xsalsa20poly1305 which is compatible
    with libnacl and libsodium.

    See :ref:`encrypted-vars` for more info.

.. opt:: workdir

    The working directory for target process. Default is ``/``. Working
    directory must be absolute.

.. opt:: resolv-conf

    Parameters of the ``/etc/resolv.conf`` file to generate. Default
    configuration is:

    .. code-block:: yaml

        resolv-conf:
            mount: nil  # which basically means "auto"
            copy-from-host: true

    Which means ``resolv.conf`` from host where lithos is running is copied
    to the "state" directory of the container. Then if ``/etc/resolv.conf``
    in container is a file (and not a symlink) resolv conf is mounted over
    the ``/etc/resolv.conf``.

    More options are expected to be added later.

    .. versionchanged:: 0.15.0

       ``mount`` option added. Previously to make use of ``resolv.conf`` you
       should symlink ``ln -s /state/resolv.conf /etc/resolv.conf`` in the
       container's image.

       Another change is that ``copy-from-host`` copies file that is specified
       in sandbox's ``resolv.conf`` which default to ``/etc/resolv.conf`` but
       may be different.

   Parameters:

   copy-from-host
        (default ``true``) Copy ``resolv.conf`` file from host machine.

        Note: even if ``copy-from-host`` is ``true``, :opt:`additional-hosts`
        from sandbox config work, which may lead to duplicate or conflicting
        entries if some names are specified in both places.

        .. versionchanged:: v0.11.0

           The parameter used to be ``false`` by default, because we were
           thinking about better (perceived) isolation.

   mount
       (default ``nil``, which means "auto") Mount copied ``resolv.conf`` file
       over ``/etc/resolf.conf``.

       `nil` enables mounting if ``/etc/resolv.conf`` is present
       in the container and is a file (not a symlink) and also
       ``copy-from-host`` is true

       .. versionadded:: 0.15.0


.. opt:: hosts-file

    Parameters of the ``/etc/hosts`` file to generate. Default
    configuration is::

        hosts-file:
            mount: nil  # which basically means "auto"
            localhost: true
            public-hostname: true
            copy-from-host: false

    .. versionchanged:: 0.15.0

       ``mount`` option added. Previously to make use of ``resolv.conf`` you
       should symlink ``ln -s /state/resolv.conf /etc/resolv.conf`` in the
       container's image.

       Another change is that ``copy-from-host`` copies file that is specified
       in sandbox's ``resolv.conf`` which default to ``/etc/resolv.conf`` but
       may be different.

   Parameters:

   copy-from-host
        (default ``true``) Copy hosts file from host machine.

        Note: even if ``copy-from-host`` is ``true``, :opt:`additional-hosts`
        from sandbox config work, which may lead to duplicate or conflicting
        entries if some names are specified in both places.

        .. versionchanged:: v0.11.0

           The parameter used to be ``false`` by default, because we were
           thinking about better (perceived) isolation. And also because
           hostname in Ubuntu doesn't resolve to real IP of the host. But we
           find those occassions where it matters to be quite rare in practice
           and using ``hosts-file`` as well as ``resolv.conf`` from the host
           system as the most expected and intuitive behavior.

   mount
       (default ``nil``, which means "auto") Mount produced ``hosts`` file over
       ``/etc/hosts``.

       `nil` enables mounting if ``/etc/hosts`` is present in the container
       and is a file (not a symlink).

       Value of ``true`` fails if ``/etc/hosts`` is not a file. Value of
       ``false`` leaves ``/etc/hosts`` intact.

       .. versionadded:: 0.15.0

   localhost
        (default is true when ``copy-from-host`` is false)
        A boolean which defines whether to add
        ``127.0.0.1 localhost`` record to ``hosts``

   public-hostname
        (default is true when ``copy-from-host`` is false)
        Add to ``hosts`` file the result of ``gethostname`` system call
        along with the ip address that name resolves into.

.. opt:: uid-map, gid-map

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

    .. note:: Currently you may have uid-map either in a sandbox or in a
       container config, not both.

.. opt:: stdout-stderr-file

    This redirects both stdout and stderr to a file. The path is opened inside
    the container. So must reside on one of the mounted writeable
    :ref:`volumes`. Probably you want :volume:`Persistent` volume.
    While it can be on :volume:`Tmpfs` or :volume:`Statedir` the applicability
    of such thing is very limited.

    Usually log is put into the directory specified by :opt:`stdio-log-dir`.

.. opt:: interactive

    (default ``false``) Useful only for containers of kind ``Command``. If
    ``true`` lithos_cmd doesn't clobber stdin and doesn't redirect stdout and
    stderr to a log file, effectively allowing command to be used for
    interactive commands or as a part of pipeline.

    .. note:: for certain use cases, like pipelines it might be better to use
       fifo's (see ``man mkfifo``) and a ``Daemon`` instead of this one
       because daemons may be restarted on death or for software upgrade,
       while ``Command`` is not supervised by lithos.

    .. versionadded:: 0.6.3

    .. versionchanged:: ≥0.5
       Commands were always interactive

.. opt:: restart-process-only

    (default ``false``) If true when restarting process (i.e. in case
    process died or was killed), lithos restarts just the failed process.
    This means container will not be recreated, volumes will not be remounted,
    tmpfs will not be cleaned and some daemon processes may leave running.

    By default ``lithos_knot`` which is pid 1 in the container exits when
    process dies. Which means all other processes will die on ``KILL`` signal,
    and container will be removed and created again. It's a little bit slower
    but safer default. This leaves no hanging daemons, orphan files in state
    dir and tmpfs garbage.

.. opt:: volumes

    The mapping of mountpoint to volume definition. See :ref:`volumes` for more
    info

.. opt:: tcp-ports

    Binds address and provides file descriptor to the child process. All the
    children receive dup of the same file descriptor,
    so may all do ``accept()`` simultaneously. The configuration looks like::

        tcp-ports:
          7777:
            fd: 3
            host: 0.0.0.0
            listen-backlog: 128
            reuse-addr: true
            reuse-port: false

    All the fields except ``fd`` are optional.

    Programs may require to pass listening file descriptor number by some
    means (usually environment). For example to run nginx with port bound
    (so you don't need to start it as root) you need::

        tcp-ports:
          80:
            fd: 3
        environ:
          NGINX: "3;"

    To run gunicorn you may want::

        tcp-ports:
          80:
            fd: 3
        environ:
          GUNICORN_FD: "3"

    More examples are in :ref:`tcp-ports-tips`

    Parameters:

    *key*
      TCP port number.

      .. warning::

         * The paramters (except ``fd``) do not change after socket is
           bound even if configuration change
         * You can't bind same port with different hostnames in a
           **single process** (previously there was a global limit for the
           single port for whole lithos master, currently this is limited
           just because ``tcp-ports`` is a mapping)

      Port parameter should be unique amongst all containers. But sharing
      port works because it is useful if you are doing smooth software
      upgrade (i.e. you have few old processes running and few new processes
      running both sharing same port/file-descriptor). *Running them on single
      port is not the best practices for smooth software upgrade but that
      topic if out of scope of this documentation.*

    fd
      *Required*. File descriptor number

    host
      (default is ``0.0.0.0`` meaning all addresses) Host to bind to. It must
      be IP address, hostname is not supported.

    listen-backlog
      (default ``128``) the value to pass to the `listen()` system call. The
      value is capped by ``net.core.somaxconn``

    reuse-addr
      (default ``true``) Sets ``SO_REUSEADDR`` socket option

    reuse-port
      (default ``false``) If set to ``true`` this changes behavior of the
      lithos with respect of the socket. In default case lithos binds socket
      as quick as possible and passes to each child on start. When this set
      to ``true``, lithos creates a separate socket and calls bind for each
      process start. This has two consequences:

      * Socket is not bound when no processes started (i.e. they are failing)
      * Each process gets separate in-kernel queue of connections to accept

      This should be set to ``true`` only on very high performant servers that
      experience assymetric workload in default case.

    external
      (default ``false``) If set to ``true`` listen on the port in the
      external network (host network of the system not bridged network).
      This is only effective if :opt:`bridged-network` is enabled
      for container.

      .. versionchanged:: 0.17.0

         Previously we only allowed external ports to be declared in lithos
         config. It was expected that container in bridged network can
         listen port itself. But it turned out file descriptors are still
         convenient for some use-cases even inside a bridge.

.. opt:: metadata

   (optional) Allows to add arbitrary metadata to lithos configuration file.
   Lithos does not use and does not validate this data in any way (except that
   it must be a valid YAML). The metadata can be used by other tools that
   inspect lithos configs and extract data from it. In particular, we use
   metadata for our deployment tools (to keep configuration files
   more consolidated instead of keeping then in small fragments).

.. opt:: normal-exit-codes

   (optional) A list of exit codes which are considered normal for process
   death. This currently only improves ``failures`` metric.
   See :ref:`Determining Failure <failures>`.

   Note: by default even ``0`` exit code is considered an error for daemons,
   and for commands (``lithos_cmd``) ``0`` is considered successful.

   This setting is intended for daemons which may voluntarily exit for some
   reason (soft memory limit, version upgrade, configuration reload).

   It's not recommended to add `0` or `1` into the list, as some commands
   threat them pretty arbitrarily. For example `0` is exit code of most
   utilities running `--help` so this mistake will not be detected. And `1`
   is used for arbitrary crashes in scripting languages. So the good idea
   is to define some specific code in range of `8..120` to define successful
   exit.


.. _integer-units: http://rust-quire.readthedocs.io/en/latest/user.html#units
