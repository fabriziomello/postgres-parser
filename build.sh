#! /bin/bash
#
#    Copyright (c) 2020, ZomboDB, LLC
#
#    Permission to use, copy, modify, and distribute this software and its documentation for any purpose, without fee, and
#    without a written agreement is hereby granted, provided that the above copyright notice and this paragraph and the
#    following two paragraphs appear in all copies.
#
#    IN NO EVENT SHALL ZomboDB, LLC BE LIABLE TO ANY PARTY FOR DIRECT, INDIRECT, SPECIAL, INCIDENTAL, OR CONSEQUENTIAL
#    DAMAGES, INCLUDING LOST PROFITS, ARISING OUT OF THE USE OF THIS SOFTWARE AND ITS DOCUMENTATION, EVEN IF ZomboDB, LLC
#    HAS BEEN ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#
#    ZomboDB, LLC SPECIFICALLY DISCLAIMS ANY WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
#    MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. THE SOFTWARE PROVIDED HEREUNDER IS ON AN "AS IS" BASIS, AND
#    ZomboDB, LLC HAS NO OBLIGATIONS TO PROVIDE MAINTENANCE, SUPPORT, UPDATES, ENHANCEMENTS, OR MODIFICATIONS.
#

set
set -x

TARGET_DIR=${CARGO_TARGET_DIR}
if [ "x${TARGET_DIR}" == "x" ] ; then
  TARGET_DIR="${PWD}/target/"
fi
UNAME=$(uname)
MANIFEST_DIR="${PWD}"
PGVER="12.3"
POSTGRES_PARSER_A="${TARGET_DIR}/libpostgres_parser.a"
POSTGRES_BC="${TARGET_DIR}/postgres.bc"
BUILD_DIR="${TARGET_DIR}/${PGVER}-build"
POSTGRES_LL="${BUILD_DIR}/postgresql-${PGVER}/src/backend/postgres.ll"
INSTALL_DIR="${BUILD_DIR}/postgresql-${PGVER}/temp-install/"

if [ "x${NUM_CPUS}" == "x" ]; then
    NUM_CPUS="1"
fi

if [ -f "${POSTGRES_PARSER_A}" ] && [ -d "${INSTALL_DIR}" ]; then
  # we already have libpostgres_parser.a, so don't bother generating it again
  echo "${POSTGRES_PARSER_A};${INSTALL_DIR}"
  exit 0
fi

if [ ! -f "${POSTGRES_LL}" ] ; then
  mkdir -p "${BUILD_DIR}" || exit 1
  cd "${BUILD_DIR}" || exit 1

  # download/untar Postgres
  if [ ! -f postgresql-${PGVER}.tar.bz2 ] ; then
    wget -q https://ftp.postgresql.org/pub/source/v12.3/postgresql-${PGVER}.tar.bz2 || exit 1
  fi
  tar xjf postgresql-${PGVER}.tar.bz2 || exit 1

  # patch its Makefiles
  cd postgresql-${PGVER} || exit 1
  patch -p1 < ../../../patches/makefiles-${PGVER}.patch || exit 1

  # configure, build, and (locally) install Postgres
  if [ "x${UNAME}" == "xLinux" ] ; then
    # linux needs to use the "gold" linker
    mkdir build_bin || exit 1
    ln -s /usr/bin/ld.gold build_bin/ld || exit 1
    CFLAGS="-B${PWD}/build_bin"
  fi

  # configure postgres
  AR="llvm-ar" \
  CC="clang" \
  CFLAGS="${CFLAGS} -O0 -flto" \
  ./configure --without-readline --without-zlib --prefix="${INSTALL_DIR}" || exit 1

  # and compile/install it
  make -j${NUM_CPUS} clean || exit 1
  make -j${NUM_CPUS} || exit 1
  rm -rf "${INSTALL_DIR}" || exit 1
  make install || exit 1

  # adjust comment style so Rust's 'bindgen' will pick them up
  # we do this against the headers in the ${INSTALL_DIR} as we
  # don't want to risk messing up original Postgres sources
  for f in "${INSTALL_DIR}/include/server/nodes/parsenodes.h" "${INSTALL_DIR}/include/server/nodes/primnodes.h" ; do
    sed -i'' -e 's/\/\*/\/**/g' "$f" || exit 1  # C-style comments start with two asterisks
    sed -i'' -e 's/-//g' "$f" || exit 1         # remove consecutive dashes
    sed -i'' -e "s/\`/'/g" "$f" || exit 1     # backticks to single quotes

    # tabs to three spaces
    expand -t 3 "$f" > "$f.expand" || exit 1
    rm "$f" || exit 1
    mv "$f.expand" "$f" || exit 1
  done

  cd "${MANIFEST_DIR}" || exit 1
fi

# optimize postgres.ll into bitcode while picking out only the symbols we need
opt "${POSTGRES_LL}" -o "${POSTGRES_BC}" \
  -internalize \
  -internalize-public-api-file=./symbols.txt \
  -globaldce \
  -O2 || exit 1

# compile into a native object
clang -c -fPIC "${POSTGRES_BC}" -o "${TARGET_DIR}/raw_parser.o"

# create an archive which the Rust crate will statically link
llvm-ar crv "${POSTGRES_PARSER_A}" "${TARGET_DIR}/raw_parser.o" || exit 1
echo "${POSTGRES_PARSER_A};${INSTALL_DIR}"
