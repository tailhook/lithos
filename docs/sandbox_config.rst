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

.. opt:: allow-groups

   List of ranges of group ids for the container.
   Works similarly to :opt:`allow-users`.

.. opt:: allow-tcp-ports

   List of ranges of allowed TCP ports for container. This is currently not
   enforced in any way except:

   1. Ports < 1024 are restricted by OS for non-root (but may be allowed here)
   2. It restricts :opt:`bind-port` setting in container config

   .. note:: if you have overlapping TCP port for different sandboxes, only
      single file descriptor will be used for each port. The config for
      opening port will be used arbitrary from single config amonst all users,
      which have obvious security implications.

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


