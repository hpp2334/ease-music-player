import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/models/current_storage_entries.dart';
import 'package:ease_music_player/models/storage_list.dart';
import 'package:ease_music_player/widgets/screen_container.dart';
import 'package:flutter/material.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';

import '../../global.dart';
import '../../services/router.service.dart';
import '../../widgets/button.dart';
import '../../widgets/icons.dart';

bool _isStateError(CurrentStorageStateType stateType) {
  return stateType == CurrentStorageStateType.NeedPermission ||
      stateType == CurrentStorageStateType.AuthenticationFailed ||
      stateType == CurrentStorageStateType.UnknownError ||
      stateType == CurrentStorageStateType.Timeout;
}

class ImportFromStorageEntriesPage extends StatefulWidget {
  final CurrentStorageImportType importType;
  const ImportFromStorageEntriesPage({super.key, required this.importType});

  @override
  ImportFromStorageEntriesPageState createState() =>
      ImportFromStorageEntriesPageState();
}

class ImportFromStorageEntriesPageState
    extends State<ImportFromStorageEntriesPage> {
  List<String> undoStack = List.empty(growable: true);

  void checkFile(VCurrentStorageEntry item) {
    bridge.scope((api) => api.selectEntry(arg: item.path));
  }

  void handleImport() {
    bridge.scope((api) => api.finishSelectedEntriesInImport());
    routerService.back();
  }

  @override
  Widget build(BuildContext context) {
    final state = context.watch<CurrentStorageEntriesModel>().value;
    final storagesState = context.watch<StorageListModel>().value;
    final selectedCount = state.selectedCount;
    final stateType = state.stateType;

    bool canPopScope() {
      return undoStack.isEmpty;
    }

    void popStack() {
      if (undoStack.isEmpty) {
        return;
      }
      final last = undoStack.removeLast();
      bridge.scope((api) => api.locateEntry(arg: last));
    }

    return PopScope(
      canPop: canPopScope(),
      onPopInvoked: (bool didPop) async {
        if (didPop) {
          return;
        }
        return popStack();
      },
      child: EaseScreenContainer(
        hidePlayer: true,
        child: Stack(
          children: [
            Positioned(
              left: 0,
              right: 0,
              top: 0,
              bottom: 0,
              child: Column(
                mainAxisAlignment: MainAxisAlignment.start,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _ImportFromStoragesHeader(
                    state: state,
                    selectedCount: selectedCount,
                    onTapBack: () {
                      if (canPopScope()) {
                        routerService.back();
                      } else {
                        popStack();
                      }
                    },
                    onLongPressBack: () {
                      routerService.back();
                    },
                  ),
                  const SizedBox(height: 10),
                  SizedBox(
                    height: 65,
                    child: StoragePickerInImportFromStorageEntries(
                      storagesState: storagesState,
                      currentStorageId: state.currentStorageId,
                    ),
                  ),
                  if (stateType == CurrentStorageStateType.Loading)
                    const Expanded(
                      child: Padding(
                        padding: EdgeInsets.symmetric(
                          horizontal: 28,
                          vertical: 10,
                        ),
                        child: StorageSkeletonInImportFromStorageEntries(),
                      ),
                    ),
                  if (_isStateError(stateType))
                    _ImportFromStoragesErrorWidget(stateType: stateType),
                  if (stateType == CurrentStorageStateType.OK)
                    _ImportFromStoragesContentWidget(
                      state: state,
                      onTapEntry: (item) {
                        if (item.isFolder) {
                          undoStack.add(state.currentPath);
                          bridge
                              .scope((api) => api.locateEntry(arg: item.path));
                        } else {
                          bridge
                              .scope((api) => api.selectEntry(arg: item.path));
                        }
                      },
                      onTapNavigationPath: (path) {
                        undoStack.add(state.currentPath);
                        bridge.scope((api) => api.locateEntry(arg: path));
                      },
                    ),
                ],
              ),
            ),
            if (selectedCount > 0)
              Positioned(
                right: 23,
                bottom: 47,
                child: EaseIconButton(
                  color: EaseColors.primary,
                  iconColor: EaseColors.surface,
                  iconToken: EaseIconsTokens.ok,
                  size: EaseIconButtonSize.Medium,
                  onTap: (_) {
                    handleImport();
                  },
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _ImportFromStoragesContentWidget extends StatelessWidget {
  const _ImportFromStoragesContentWidget({
    required this.state,
    required this.onTapEntry,
    required this.onTapNavigationPath,
  });

  final VCurrentStorageEntriesState state;
  final void Function(VCurrentStorageEntry) onTapEntry;
  final void Function(String) onTapNavigationPath;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 10),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            _ImportFromStoragesNavigation(
              state: state,
              onTapNavigationPath: onTapNavigationPath,
            ),
            Expanded(
              child: ListView.builder(
                itemCount: state.entries.length,
                shrinkWrap: true,
                padding: EdgeInsets.zero,
                itemBuilder: (context, index) {
                  final item = state.entries[index];
                  final itemIcon = (() {
                    switch (item.entryTyp) {
                      case StorageEntryType.Music:
                        return EaseIconsTokens.musicNote;
                      case StorageEntryType.Folder:
                        return EaseIconsTokens.folder;
                      case StorageEntryType.Image:
                      case StorageEntryType.Lyric:
                      case StorageEntryType.Other:
                        return EaseIconsTokens.file;
                    }
                  })();

                  return Material(
                    color: EaseColors.surface,
                    child: InkWell(
                      onTap: () {
                        onTapEntry(item);
                      },
                      child: SizedBox(
                        height: 45,
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Expanded(
                              child: Row(
                                mainAxisAlignment: MainAxisAlignment.start,
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  EaseIcon(
                                    color: EaseColors.primaryText,
                                    iconToken: itemIcon,
                                    size: 24,
                                  ),
                                  const SizedBox(width: 7),
                                  Expanded(
                                    child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      mainAxisSize: MainAxisSize.min,
                                      children: [
                                        Text(
                                          item.name,
                                          style: const TextStyle(
                                            fontSize: 14,
                                            color: EaseColors.primaryText,
                                            overflow: TextOverflow.ellipsis,
                                          ),
                                          maxLines: 1,
                                        ),
                                      ],
                                    ),
                                  ),
                                  const SizedBox(width: 10),
                                ],
                              ),
                            ),
                            if (item.isFolder)
                              const EaseIcon(
                                color: EaseColors.primaryText,
                                iconToken: EaseIconsTokens.forward,
                                size: 12,
                              ),
                            if (item.canCheck)
                              Container(
                                width: 16,
                                height: 16,
                                decoration: BoxDecoration(
                                  border: item.checked
                                      ? null
                                      : Border.all(
                                          color: EaseColors.primaryText,
                                          width: 1.5,
                                        ),
                                  borderRadius: BorderRadius.circular(2),
                                  color:
                                      !item.checked ? null : EaseColors.primary,
                                ),
                                child: !item.checked
                                    ? null
                                    : Center(
                                        child: EaseIcon(
                                          color: item.checked
                                              ? EaseColors.surface
                                              : EaseColors.primaryText,
                                          iconToken: EaseIconsTokens.ok,
                                          size: 12,
                                        ),
                                      ),
                              )
                          ],
                        ),
                      ),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ImportFromStoragesNavigation extends StatelessWidget {
  const _ImportFromStoragesNavigation({
    required this.state,
    required this.onTapNavigationPath,
  });

  final VCurrentStorageEntriesState state;
  final void Function(String) onTapNavigationPath;

  @override
  Widget build(BuildContext context) {
    const navItemSep = Text(
      ">",
      style: TextStyle(
        fontSize: 10,
        color: EaseColors.primaryText,
      ),
    );

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 28),
            child: ListView.builder(
              itemCount: state.splitPaths.length + 1,
              shrinkWrap: true,
              padding: EdgeInsets.zero,
              scrollDirection: Axis.horizontal,
              itemBuilder: (context, index) {
                if (index == 0) {
                  return EaseTextButton(
                    onPressed: () {
                      onTapNavigationPath("/");
                    },
                    text: "Root",
                    small: true,
                    style: EaseTextButtonStyle.Default,
                    disabled: index == state.splitPaths.length,
                  );
                }

                final item = state.splitPaths[index - 1];
                final name = item.name;
                return Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: [
                    navItemSep,
                    EaseTextButton(
                      onPressed: () {
                        onTapNavigationPath(item.path);
                      },
                      text: name,
                      small: true,
                      style: EaseTextButtonStyle.Default,
                      disabled: index == state.splitPaths.length,
                    ),
                  ],
                );
              },
            ),
          ),
        ),
      ],
    );
  }
}

class _ImportFromStoragesErrorWidget extends StatelessWidget {
  const _ImportFromStoragesErrorWidget({
    required this.stateType,
  });

  final CurrentStorageStateType stateType;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 80),
          child: StorageErrorInImportFromStorageEntries(
            stateType: stateType,
          ),
        ),
      ),
    );
  }
}

class _ImportFromStoragesHeader extends StatelessWidget {
  const _ImportFromStoragesHeader({
    required this.state,
    required this.selectedCount,
    required this.onTapBack,
    required this.onLongPressBack,
  });

  final VCurrentStorageEntriesState state;
  final int selectedCount;
  final void Function() onTapBack;
  final void Function() onLongPressBack;

  String _getHeaderText() {
    switch (state.importType) {
      case CurrentStorageImportType.Musics:
        {
          return state.selectedCount == 0
              ? "Import Musics"
              : "$selectedCount ${selectedCount == 1 ? "Music" : "Musics"} Selected";
        }
      case CurrentStorageImportType.CreatePlaylistEntries:
        {
          return state.selectedCount == 0
              ? "Import Entries"
              : "$selectedCount ${selectedCount == 1 ? "Entry" : "Entries"} Selected";
        }
      case CurrentStorageImportType.CurrentMusicLyrics:
        {
          return state.selectedCount == 0 ? "Import Lyric" : "Lyric Selected";
        }
      case CurrentStorageImportType.EditPlaylistCover:
      case CurrentStorageImportType.CreatePlaylistCover:
        {
          return state.selectedCount == 0 ? "Import Cover" : "Cover Selected";
        }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 14.0),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Row(
            children: [
              EaseIconButton(
                iconColor: EaseColors.primaryText,
                iconToken: EaseIconsTokens.back,
                size: EaseIconButtonSize.Small,
                onTap: (_) {
                  onTapBack();
                },
                onLongPress: (_) {
                  onLongPressBack();
                },
              ),
              Text(
                _getHeaderText(),
                style: const TextStyle(
                  fontSize: 16,
                  color: EaseColors.primaryText,
                ),
              )
            ],
          ),
          EaseIconButton(
            iconColor: state.disabledToggleAll
                ? EaseColors.disabled
                : EaseColors.primaryText,
            iconToken: EaseIconsTokens.toggleAll,
            size: EaseIconButtonSize.Small,
            disabled: state.disabledToggleAll,
            onTap: (_) {
              bridge.scope((api) => api.toggleAllCheckedEntries());
            },
          )
        ],
      ),
    );
  }
}

class StorageErrorInImportFromStorageEntries extends StatelessWidget {
  final CurrentStorageStateType stateType;

  const StorageErrorInImportFromStorageEntries({
    super.key,
    required this.stateType,
  });

  @override
  Widget build(BuildContext context) {
    final String errorText = (() {
      assert(_isStateError(stateType));
      switch (stateType) {
        case CurrentStorageStateType.NeedPermission:
          return "PERMISSION NEED";
        case CurrentStorageStateType.AuthenticationFailed:
          return "AUTHENTICATION FAIL";
        case CurrentStorageStateType.Timeout:
          return "CONNECTION TIMEOUT";
        case CurrentStorageStateType.UnknownError:
        case CurrentStorageStateType.OK:
        case CurrentStorageStateType.Loading:
          return "UNKNOWN ERROR";
      }
    })();

    final String errorDetail = (() {
      assert(_isStateError(stateType));
      switch (stateType) {
        case CurrentStorageStateType.NeedPermission:
          return "In order to read files and folders on your device. Permission should be granted.";
        case CurrentStorageStateType.AuthenticationFailed:
          return "The username or password is wrong. Please check the username and password in the configuration.";
        case CurrentStorageStateType.Timeout:
          return "Connect timeout. Please try again.";
        case CurrentStorageStateType.UnknownError:
        case CurrentStorageStateType.OK:
        case CurrentStorageStateType.Loading:
          return "Unknown error. Please try again.";
      }
    })();

    final iconToken = stateType == CurrentStorageStateType.NeedPermission
        ? EaseIconsTokens.fileManaged
        : EaseIconsTokens.warning;

    return GestureDetector(
      onTap: () async {
        if (stateType == CurrentStorageStateType.NeedPermission) {
          await Permission.manageExternalStorage.request();
          await bridge.syncStoragePermission();
        }
        bridge.scope((api) => api.refreshCurrentStorageInImport());
      },
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 60,
            height: 60,
            decoration: BoxDecoration(
              color: EaseColors.error,
              borderRadius: BorderRadius.circular(60),
            ),
            child: Center(
              child: EaseIcon(
                color: EaseColors.surface,
                iconToken: iconToken,
                size: 32,
              ),
            ),
          ),
          const SizedBox(height: 9),
          Text(
            errorText,
            style: const TextStyle(
              color: EaseColors.error,
              fontSize: 14,
              fontWeight: FontWeight.bold,
            ),
          ),
          const SizedBox(height: 9),
          Text(
            errorDetail,
            style: const TextStyle(
              color: EaseColors.secondaryText,
              fontSize: 12,
            ),
          )
        ],
      ),
    );
  }
}

class StorageSkeletonInImportFromStorageEntries extends StatelessWidget {
  const StorageSkeletonInImportFromStorageEntries({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    final iconSkeleton = Container(
      width: 29,
      height: 29,
      decoration: BoxDecoration(
        color: EaseColors.light,
        borderRadius: BorderRadius.circular(6),
      ),
    );
    final checkboxSkeleton = Container(
      width: 16,
      height: 16,
      decoration: BoxDecoration(
        color: EaseColors.light,
        borderRadius: BorderRadius.circular(6),
      ),
    );
    final titleSkeleton = Container(
      width: 138,
      height: 16,
      decoration: BoxDecoration(
        color: EaseColors.light,
        borderRadius: BorderRadius.circular(6),
      ),
    );
    final subTitleSkeleton = Container(
      width: 45,
      height: 9,
      decoration: BoxDecoration(
        color: EaseColors.light,
        borderRadius: BorderRadius.circular(6),
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        titleSkeleton,
        const SizedBox(height: 14),
        Row(
          children: [
            iconSkeleton,
            const SizedBox(width: 10),
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                titleSkeleton,
                const SizedBox(height: 3),
                subTitleSkeleton,
              ],
            )
          ],
        ),
        const SizedBox(height: 12),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Row(
              children: [
                iconSkeleton,
                const SizedBox(width: 10),
                titleSkeleton
              ],
            ),
            checkboxSkeleton,
          ],
        ),
        const SizedBox(height: 12),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Row(
              children: [
                iconSkeleton,
                const SizedBox(width: 10),
                titleSkeleton
              ],
            ),
            checkboxSkeleton,
          ],
        ),
        const SizedBox(height: 12),
        Row(
          children: [iconSkeleton, const SizedBox(width: 10), titleSkeleton],
        ),
      ],
    );
  }
}

class StoragePickerInImportFromStorageEntries extends StatelessWidget {
  const StoragePickerInImportFromStorageEntries({
    super.key,
    required this.storagesState,
    required this.currentStorageId,
  });

  final VStorageListState storagesState;
  final StorageId? currentStorageId;

  @override
  Widget build(BuildContext context) {
    return ListView.separated(
      itemCount: storagesState.items.length + 3,
      scrollDirection: Axis.horizontal,
      shrinkWrap: true,
      separatorBuilder: (context, index) => const SizedBox(width: 13),
      itemBuilder: (context, index) {
        if (index == storagesState.items.length + 1) {
          return SizedBox(
            width: 65,
            height: 65,
            child: Material(
              color: EaseColors.light,
              borderRadius: BorderRadius.circular(10.0),
              child: InkWell(
                borderRadius: BorderRadius.circular(10.0),
                onTap: () {
                  bridge.scope((api) => api.prepareEditStorage());
                  routerService.goEditStorage(null);
                },
                child: const Center(
                  child: EaseIcon(
                    color: EaseColors.primaryText,
                    iconToken: EaseIconsTokens.plus,
                    size: 16,
                  ),
                ),
              ),
            ),
          );
        }
        if (index == 0 || index == storagesState.items.length + 2) {
          return const SizedBox(width: 15);
        }

        final item = storagesState.items[index - 1];
        final isActive = item.storageId.field0 == currentStorageId?.field0;
        final textColor =
            isActive ? EaseColors.surface : EaseColors.primaryText;

        return SizedBox(
          width: 142,
          height: 65,
          child: Stack(
            children: [
              Positioned.fill(
                child: Material(
                  color: isActive ? EaseColors.primary : EaseColors.light,
                  borderRadius: BorderRadius.circular(10.0),
                  child: InkWell(
                    borderRadius: BorderRadius.circular(10.0),
                    onTap: () {
                      if (!isActive) {
                        bridge.scope((api) =>
                            api.selectStorageInImport(arg: item.storageId));
                      }
                    },
                    child: Container(
                      padding: const EdgeInsets.all(15),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            item.name,
                            style: TextStyle(
                              color: textColor,
                              fontSize: 14,
                              overflow: TextOverflow.ellipsis,
                            ),
                            maxLines: 1,
                          ),
                          Text(
                            item.subTitle,
                            style: TextStyle(
                              color: textColor,
                              fontSize: 10,
                              overflow: TextOverflow.ellipsis,
                            ),
                            maxLines: 1,
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
              if (item.typ != StorageType.Local)
                Positioned(
                  right: -4,
                  bottom: -8,
                  child: Opacity(
                    opacity: 0.3,
                    child: EaseIcon(
                      iconToken: EaseIconsTokens.cloud,
                      size: 28,
                      color: isActive
                          ? EaseColors.surface
                          : EaseColors.primaryText,
                    ),
                  ),
                )
            ],
          ),
        );
      },
    );
  }
}
