# ATOF("4") = 4.0

Welcome to the jungle

## Setup tools

### Basics

After cloning the repository, go ahead and `./setup.sh` all the tools. To point
your system at those tools (after install and before any development session),
you need to `source activate.sh`.

Once that is done, you will be able to proceed with building/testing the whole
stack without any further need for an internet connexion.

### Parameters

`./setup.sh` defines defaults for where to install tools, which version to
install etc. You can override this using environment variables. To see the list
of knobs and their defaults, `egrep '^test -z' ./setup.sh`.
