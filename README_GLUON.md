# Gluon version of libsignal

* [Introduction](#introduction)
* [Synced version from upstream](#synced-version-from-upstream)
   * [Introduction](#introduction-1)
   * [Syncing with upstream](#syncing-with-upstream)
   * [Publishing a new snapshot](#publishing-a-new-snapshot)
* [Experimental components](#experimental-components)
   * [Introduction](#introduction-2)
   * [Build process](#build-process)
   * [Producing a new full release](#producing-a-new-full-release)
   * [Syncing with upstream](#syncing-with-upstream-1)

## Introduction

The Gluon fork of [libsignal](https://github.com/signalapp/libsignal) currently contains two
components:

* Inside branch `main-upstream`: a plain synced branch with changes from main upstream
* Inside branch `main`: experimental components in rust

## Synced version from upstream

### Introduction

The branch `main-upstream` contains no code changes. It contains the latest changes from the
`main` branch of the upstream repository and makes it available as a snapshot build into the Gluon
nexus snapshot repository: https://nexus.gluonhq.com/nexus/content/repositories/public-snapshots/org/signal/libsignal-client/head-SNAPSHOT/

We build the native library for the following platforms:

* linux x64 and aarch64
* mac x64 and aarch64
* windows x64

### Syncing with upstream

To keep the branch up-to-date with upstream, you can use the github website:

* Navigate to the branch: https://github.com/gluonhq/libsignal/tree/main-upstream
* If the branch is out-of-date with upstream, you can click the button `Sync fork`
* In case there are merge conflicts, github will suggest to create a PR in a separate
  branch in which you can resolve the conflicts locally

See [Syncing a fork](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/syncing-a-fork) on the github docs for more detailed options on how to sync a
fork.

### Publishing a new snapshot

The steps for publishing a new snapshot are:

1. Check that you have the latest changes from the upstream repository
2. Trigger the workflow: [Upload Java libraries to Sonatype](https://github.com/gluonhq/libsignal/actions/workflows/jni_artifacts.yml)
   by selecting `Run workflow` from the dropdown and then select the branch `main-upstream` for the `Use workflow from` dropdown.

## Experimental components

### Introduction

The branch `main` contains experimental components written in Rust that are accessible for Java
applications through JNI. The extra components are the following:

* chat: network communication using protobuf
* grpc: network communication using gRPC
* quic: network communication using the QUIC protocol

The project contains two aspects:

* an implementation of the logic in Rust
* an interface to use the Rust components in Java

The Rust code is compiled into a native shared library for the following platforms:

* linux x64 and aarch64
* mac x64 and aarch64
* windows x64

### Build process

Everything is built from a Gradle project that is located in the `java` directory. Running a
build locally consists of the following steps:

1. Navigate into the `java` directory
2. Run the following gradle command:

    ```
    ./gradlew build publishToMavenLocal -PskipAndroid -x :client:proguard -x :client:diffUnusedProguard
    ```

3. After the build succeeded, two jars should be deployed to the local maven repository:
    1. One that contains the java classes
    2. One that contains the native shared library targeted for the current platform

### Producing a new full release

We use Github Actions to create a release by using the workflow `jni_artifacts.yml`. This
workflow is triggered manually. The workflow is split into two jobs. The first job generates the
native shared library for all the platforms except linux x64. Each shared library is uploaded as
an artifact. The second job then builds the native shared library for linux x64, downloads the
previously generated native shared libraries for the other platforms and compiles the java
classes. It then creates one jar with the java classes and separate jars for each platform
containing only the respective native shared library. The final step is to deploy these jars to
the Gluon nexus repository.

The steps for building a new release are:

1. Update the version at the top of `java/build.gradle`, e.g. `0.67.6-gluon-1`
2. Commit and push the changes
3. Create a tag that matches the version and push it, e.g. `v0.67.5-gluon-1`
4. Trigger the workflow: [Upload Java libraries to Sonatype](https://github.com/gluonhq/libsignal/actions/workflows/jni_artifacts.yml)
by selecting `Run workflow` from the dropdown and then select the tag for the `Use workflow from`
dropdown.

### Syncing with upstream

This is the process for syncing with a specific release from the upstream repository:

1. Choose a version that we need to sync to
2. Create a branch from main called `patch-VERSION`, e.g. `patch-v0.67.5`
3. Fetch everything from upstream: `git fetch upstream`
4. Merge the tagged commit with the current branch: `git merge upstream v0.67.5`
5. Resolve any conflicts
6. Update the version in `java/build.gradle` to match the version that we synced with
7. Run a build as described above