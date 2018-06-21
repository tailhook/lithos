.. _sandbox_config:

==============
Sandbox Config
==============


This config resides in ``/etc/lithos/sandboxes/NAME.yaml`` (by default).
Where ``NAME`` is the name of a sandbox.

The configuration file contains security and resource limits for the container.
Including:

* A directory where image resides
* Set of directories that are mounted inside the container (i.e. all writable
  directories for the container, the ``/tmp``...)
* ulimit settings
* cgroup limits

Reference
=========


.. opt:: config-file

   The path for the :ref:`processes config<process_config>`.  In most cases
   should be left unset.  Default is ``null`` which is results into
   ``/etc/lithos/processes/NAME.yaml`` with all other settings being defaults.

.. opt:: image-dir

   Directory where application images are. Every subdir of the ``image-dir``
   may be mounted as a root file system in the container. **Required**.

.. opt:: image-dir-levels

   (default ``1``) A number of directory components required for image name
   in :opt:`image-dir`

   .. versionadded: 0.12.1

.. opt:: log-file

   The file name where to put **supervisor** log of the container. Default is
   ``/var/log/lithos/SANDBOX_NAME.yaml``.

.. opt:: log-level

   (default ``warn``). The logging level of the supervisor.

.. opt:: readonly-paths

   The mapping of ``virtual_directory: host_system_directory`` of folders which
   are visible for the container in read-only mode. (Note currently if you
   have submounts in the source directory, thay may be available as writeable).
   See :ref:`Volumes` for more details.

.. opt:: writable-paths

   The mapping of ``virtual_directory: host_system_directory`` of folders which
   are visible for the container in writable mode.
   See :ref:`Volumes` for more details.

.. opt:: allow-users

   List of ranges of user ids which can be used by container. For containers
   without user namespaces, it's just a limit of the ``user-id`` setting.

   Example::

     allow-users: [1, 99, 1000-2000]

   For containers which have uid maps enabled **in sandbox** this is a list of
   users available *after* uid mapping applied. For example, the following
   maps uid 100000 as root in namespace (e.g. for file permissions),
   but doesn't allow to start process as root (even if it's 100000 ouside)::

     uid-map: [{outside: 100000, inside: 0, count: 65536}]
     allow-users: [1-65535]

   For containers which do have uid maps enabled **in container config**,
   it limits all the user ids available to the namespace (i.e. for the
   outside setting of the uid map).

.. opt:: default-user

   (no default) A user id used in the container if no ``user-id`` is specified
   in container config. By default ``user-id`` is required.

   Note: ``default-user`` value must be contained in the ``allow-users`` range

   .. versionadded: v0.15.3

.. opt:: allow-groups

   List of ranges of group ids for the container.
   Works similarly to :opt:`allow-users`.

.. opt:: default-group

   (default ``0``) A group id used in the container if no ``group-id``
   is specified in container config.

   Note: ``default-group`` value must be contained in the ``allow-users`` range

   .. versionadded: v0.15.3

      In previous versions default group was always zero.

.. opt:: allow-tcp-ports

   List of ranges of allowed TCP ports for container. This is currently not
   enforced in any way except:

   1. Ports < 1024 are restricted by OS for non-root (but may be allowed here)
   2. It restricts :opt:`bind-port` setting in container config

   .. note:: if you have overlapping TCP port for different sandboxes, only
      single file descriptor will be used for each port. The config for
      opening port will be used arbitrary from single config amonst all users,
      which have obvious security implications.

   .. warning:: :opt:`tcp-ports` bind at port in **host namespace**, i.e. it
      effectively discards :opt:`bridged-network` for that port this is both
      the feature and might be a pitfall. So most of the time you should avoid
      non-empty :opt:`allow-tcp-ports` if using `bridged-network`.

.. opt:: additional-hosts

   Mapping of ``hostname: ip`` for names that will be added to ``/etc/hosts``
   file. This is occasinally used for cheap but static service discovery.

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

.. opt:: used-images-list

    (optional) A text file that is used by ``lithos_clean`` to keep images
    alive. It's not used by any other means except ``lithos_clean`` utility.

    Each line of the file should contain image name relative to the
    ``image_dir``.

    It's expected that the list is kept up by some orchestration system or
    by deployment scripts or by any other tool meaningful for ops team.

    This setting is only useful if ``auto-clean`` is ``true`` (default)

.. opt:: auto-clean

   (default ``true``) Clean images of this sandbox when running
   ``lithos_clean``. This is a subject of the following caveats:

   1. Lithos clean is not run by lithos automatically, you ought to run it
      using cron tab
   2. If same ``image-dir`` is used for multiple sandboxes it will be cleaned
      if at least one of them has non-falsy ``auto-clean``.

.. opt:: resolv-conf

   (default ``/etc/resolv.conf``) default place to copy ``resolv.conf`` from
   for containers.

   Note: Container itself can override it's own resolv.conf file, but can't
   read original ``/etc/resolv.conf`` if this setting is changed.

.. opt:: hosts-file

   (default ``/etc/hosts``) default place to copy ``hosts`` from
   for containers.

   Note: Container itself can override it's own ``hosts`` file, but can't
   read original ``/etc/hosts`` if this setting is changed.

.. opt:: bridged-network

   (default is absent) a network bridge configuration for all the cotainers in
   the bridge

   Example:

   .. code-block:: yaml

      bridged-network:
        bridge: br0
        network: 10.0.0.0/24
        default_gateway: 10.0.0.1
        after-setup-command: [/usr/bin/arping, -U, -c1, '@{container_ip}']

   .. note:: when bridged network is active your :ref:`process_config` should
      contain a list of ip addresses one for each container.

   .. note:: this setting does not affect ``tcp-ports``. So usually you should
      keep :opt:`allow-tcp-ports` setting empty when using bridged network.

   .. versionchanged: 0.18.0

      Previously lithos always called `/usr/bin/arping` now it doesn't but
      the example of `after-setup-command` shown above does exactly same thing.

   Options:

   .. bopt:: after-setup-command

      Command to run after setting up container namespace but before running
      actual container. The example shown above sends unsolicited arp packet
      to notify router and other machines on the network that MAC address
      corresponding to container's IP is changed.

      Command must have absolute path, and has almost empty environment, so
      don't assume ``PATH`` is there if you're writing a script. Command runs
      in *container's network* namespace but with all other namespaces in host
      system (in particular in *host filesystem* and with permissions of root
      in host system)

      Replacement variables that work in command-line:

      * ``@{container_ip}`` -- replaced with IP address of a container being
        set up

      Few examples:

      1. ``[/usr/bin/arping, -U, -c1, '@{container_ip}']`` -- default
         in v0.17.x. This notifies other peers that MAC address for
         this IP changed.
      2. ``[/usr/bin/arping, -c1, '10.0.0.1']`` -- other way to do that, that
         often does the same as in (1) a side-effect
         (where 10.0.0.1 is a default gateway)
      3. ``[/usr/bin/ping, -c1, '10.0.0.1']`` -- doing same as (2) but using
         ICMP instead of ARP directly

      Most of the time containers should work with empty
      ``after-setup-command``, but because container gets new MAC address each
      time it starts, there might be a small delay (~ 5 sec) after container's
      start where packets going to that IP are lost (so it appears that host
      is unavailable).

      .. version-added: v0.18.0


.. opt:: secrets-private-key

    (default is absent) Use the specified private key(s) to decode secrets
    in container's :opt:`secret-environ` setting.

    The key in this file is openssh-compatible ed25519 private  key
    (RSA keys are *not* supported). File can contain multiple keys
    (concatenated), if secret matches any of them it will be decoded.

    To create a key use normal ``ssh-keygen`` and leave the password empty
    (password-protected keys aren't supported)::

        ssh-keygen -t ed25519 -t /etc/lithos/keys/secret.key

    Note: the key must be owned by root with permissions of 0600 (default for
    ssh-keygen).

.. opt:: secrets-namespaces

    (default is `[""]`) allow only secrets with listed namespaces.
    Useful only if ``secrets-private-key`` is set.

    For example:

    .. code-block:: yaml

        secrets-namespaces:
        - project1.web
        - project1.celery

    The idea is you might want to use single secret private key for a whole
    cluster. But diferent services having different "namespaces". This means
    you can use single public key for encyption and specify different
    namespace for each service. With this setup user can't just copy a
    key from one service to another if that another service isn't authorized
    to read the namespace using :opt:`secrets-namespaces`.

    To encrypt secret for a specific namespace use::

        lithos_crypt encrypt -k key.pub -d "secret" -n "project1.web"

    By default both ``lithos_crypt`` and :opt:`secrets-namespaces` specify
    empty string as a namespace. This is good enough if you don't have
    multiple teams sharing the same cluster.

    Currently namespaces are limited to a regexp ``^[a-zA-Z0-9_.-]*$``

    See :ref:`encrypted-vars` for more info.

