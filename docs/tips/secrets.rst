===============
Storing Secrets
===============

There are currently two ways to provide "secrets" for containers:

1. Encrypted values inserted into environment variable
2. Mount a directory from the host system

.. contents:: :local:

.. _encrypted-vars:

Encrypted Variables
===================

Guide
-----

Note: this guide covers both server setup and configuring specific containers.
Usually setup (steps 1-3) is done once. And adding keys to a container
(steps 4-5) is more regular job.

1. Create a key private key on the server::

        ssh-keygen -f /etc/lithos/keys/main.key -t ed25519 -P ""

   You can create a shared key or a per-project key.  Depending on your
   convenience. Synchronize the key accross all the servers in the same cluster.
   This key should **never leave** that set of servers.

2. Add the reference to the key into your :ref:`sandbox_config`
   (e.g. ``/etc/lithos/sandboxes/myapp.yaml``):

    .. code-block:: yaml

       secrets-private-key: /etc/lithos/keys/main.key
       secrets-namespaces: [myapp]

    You can omit ``secrets-namespaces`` if you're sole owner of this
    server/cluster (it allows only empty string as a namespace). You can also
    make per-process namespaces (:popt:`extra-secrets-namespaces`).

3. Publish your public key ``/etc/lithos/keys/main.key.pub`` for your users.
   *(Cryptography guarantees that even if this key is shared publically, i.e.
   commited into a git repo, or accessible over non-authorized web URL system
   is safe)*

4. Your users may now fetch the public key and encrypt their secrets with
   ``lithos_crypt`` (get static binary on `releases page`_):

   .. code-block:: console

        $ lithos_crypt encrypt -k main.key.pub -n myapp -d the_secret
        v2:ROit92I5:KqWSX0BY:8MtOoWUX:nHcVCIWZG2hivi0rKa8MRnAIbt7TDTHB8YC8bBnac3IGMzk57R/HsBhxeqCdC7Ljyf8pszBBjIGD33f6lwBM7Q==

   The important thing here is to encrypt with the right key **and**
   the right namespace.

5. Then put a secret into your :ref:`container_config`:

   .. code-block:: yaml

      executable: /usr/bin/python3
      environ:
        DATABASE_URL: postgresql://myappuser@db.example.com/myappdb
      secret-environ:
        DATABASE_PASSWORD: v2:ROit92I5:KqWSX0BY:8MtOoWUX:nHcVCIWZG2hivi0rKa8MRnAIbt7TDTHB8YC8bBnac3IGMzk57R/HsBhxeqCdC7Ljyf8pszBBjIGD33f6lwBM7Q==

That's it. To add a new password to the same or another container repeat
steps 4-5.

This scheme is specifically designed to be safe to store in a (public) git
repository by using secure encryption.

.. _releases page: https://github.com/tailhook/lithos/releases


.. _key-structure:

Ananomy of the Encrypted Key
----------------------------

As you might see there is a pattern in an encrypted key. Here is how it
looks like::

    v2:ROit92I5:KqWSX0BY:8MtOoWUX:nHcVCIWZG2hivi0rKa8MRnAIbt7TDTHB8YC8bBnac3IGM‥wBM7Q==
                                  ^-- encrypted "namespace:actual_secret"
                         ^^^^^^^^-- short hash of the password itself
                ^^^^^^^^-- short hash of the secrets namespace
       ^^^^^^^^-- short hash of the public key used for encryption
    ^^-- encryption version

Note the following things:

1. Only version ``v2`` is supported (``v1`` was broken and dropped in 0.16.0)

2. The short hash is base64-encoded 6-bytes length blake2b hash of the value.
   You can check in using ``b2sum`` utility from recent version of ``coreutils``:

   .. code-block:: console

       $ echo -n "the_secret" | b2sum -l48 | xxd -r -p | base64
       8MtOoWUX

   (Note: we need ``xxd`` because ``b2sum`` outputs hexadecimal bytes, also
   note ``-n`` in ``echo`` command, as it's a common mistake, without the option
   ``echo`` outputs newline at the end).

3. The encrypted payload contains ``<namespace>:`` prefix. While we could
   check just the hash. Prefix allows providing better error messages.

   The underlying encyrption is curve25519xsalsa20poly1305 which is compatible
   with libnacl and libsodium.

Let's see how it might be helpful, here is the list of keys:

.. code-block:: text
   :linenos:

   v2:h+M9Ue9x:82HdsExJ:Gd3ocJsr:/+f4ezLfKIP/mp0xdF7H6gfdM7onHWwbGFQX+M1aB+PoCNQidKyz/1yEGrwxD+i+qBGwLVBIXRqIc5FJ6/hw26CE
   v2:ROit92I5:cX9ciQzf:Gd3ocJsr:LMHBRtPFpMRRrljNnkaU6Y9JyVvEukRiDs4mitnTksNGSX5xU/zADWDwEOCOtYoelbJeyDdPhM7Q1mEOSwjeyO317Q==
   v2:ROit92I5:82HdsExJ:Gd3ocJsr:Hp3pngQZUos5b8ioKVUx40kegM1uDsYWwsWqC1cJ1/1KmQPQQWJZe86xgl1EOIxbuLj6PUlBH8yz5qCnWp//Ofbc

You can see that:

1. All of them have same secret (3rd column)
2. Second and third ones have same encryption key (1st column)
3. First and third ones have the same namespace (2nd column)

This is useful for versioning and debugging problems. You can't deduce the
actual password from this data anyway unless your password is very simple
(dictioanry attack) or you already know it.

Note: even if all three {encryption key, namespace, secret} match, the
last part of data (encrypted payload) will be different each time you encode
that same value. All of the outputs are equally right.


Security Notes
--------------

1. Namespaces allow to divide security zones between many projects without
   nightmare of generating, syncing and managing secret keys per project.
2. Namespaces match exactly they aren't prefixes or any other kind of pattern
3. If you rely on ``lithos_switch`` to switch containers securely (with
   untrusted :ref:`process_config`), you need to use different private key
   per project (as otherwise ``extra-secrets-namespaces`` can be used to steal
   keys)
