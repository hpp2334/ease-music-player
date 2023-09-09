import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/create_playlist.dart';
import 'package:ease_music_player/services/resource.service.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/widgets/form.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../widgets/button.dart';
import '../../widgets/dialog.dart';

void showCreatePlaylistDialog(BuildContext context) {
  bridge.scope((api) => api.prepareCreatePlaylist());
  showDialog(
    context: context,
    builder: (BuildContext context) {
      return const _CreatePlaylistDialog();
    },
  );
}

class _CreatePlaylistDialog extends StatefulWidget {
  const _CreatePlaylistDialog();

  @override
  _CreatePlaylistDialogState createState() => _CreatePlaylistDialogState();
}

class _CreatePlaylistDialogState extends State<_CreatePlaylistDialog> {
  final _formKey = GlobalKey<FormState>();
  final TextEditingController _titleController = TextEditingController();
  int _lastSignal = 0;
  CreatePlaylistModel? model;

  @override
  void initState() {
    super.initState();

    _titleController.addListener(() {
      bridge.scope(
          (api) => api.updateCreatePlaylistName(arg: _titleController.text));
    });

    model = context.read<CreatePlaylistModel>();
    model!.addListener(preparedSignalListener);
  }

  @override
  void dispose() {
    model!.removeListener(preparedSignalListener);
    super.dispose();
  }

  void preparedSignalListener() {
    if (model != null) {
      final createPlaylistValues = model!.value;
      if (_lastSignal != createPlaylistValues.preparedSignal) {
        _lastSignal = createPlaylistValues.preparedSignal;
        _titleController.text = createPlaylistValues.name;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = context.watch<CreatePlaylistModel>().value;

    return EaseDialog(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 18, horizontal: 24),
        child: Column(
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                CreatePlaylistNav(
                  width: 50,
                  text: "FULL",
                  isActive: state.mode == CreatePlaylistMode.Full,
                  onTapInactive: () {
                    bridge.scope((api) => api.updateCreatePlaylistMode(
                        arg: CreatePlaylistMode.Full));
                  },
                ),
                const SizedBox(width: 14),
                CreatePlaylistNav(
                  width: 50,
                  text: "EMPTY",
                  isActive: state.mode == CreatePlaylistMode.Empty,
                  onTapInactive: () {
                    bridge.scope((api) => api.updateCreatePlaylistMode(
                        arg: CreatePlaylistMode.Empty));
                  },
                ),
              ],
            ),
            const SizedBox(height: 30),
            Row(
              children: [
                Expanded(
                  child: Form(
                    key: _formKey,
                    child: CreatePlaylistForm(
                      titleController: _titleController,
                      state: state,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 30),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                if (state.mode == CreatePlaylistMode.Full)
                  Row(
                    mainAxisAlignment: MainAxisAlignment.start,
                    children: [
                      EaseTextButton(
                        onPressed: () {
                          bridge.scope((api) => api.resetCreatePlaylistFull());
                        },
                        text: 'RESET',
                      ),
                    ],
                  ),
                Expanded(
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      EaseTextButton(
                        onPressed: () {
                          routerService.back();
                        },
                        text: 'CANCEL',
                      ),
                      EaseTextButton(
                        onPressed: () {
                          if (!_formKey.currentState!.validate()) {
                            return;
                          }

                          bridge.scope((api) => api.finishCreatePlaylist());
                          routerService.back();
                        },
                        text: 'OK',
                        disabled: state.mode == CreatePlaylistMode.Full &&
                            !state.fullImported,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class CreatePlaylistForm extends StatelessWidget {
  const CreatePlaylistForm({
    super.key,
    required TextEditingController titleController,
    required this.state,
  }) : _titleController = titleController;

  final VCreatePlaylistState state;
  final TextEditingController _titleController;

  @override
  Widget build(BuildContext context) {
    final isFull = state.mode == CreatePlaylistMode.Full;
    final picture =
        state.picture != null ? resourceService.load(state.picture!) : null;

    const gapWidget = SizedBox(
      height: 24,
    );
    final playNameWidget = EaseFormText(
      controller: _titleController,
      label: "Playlist Name",
      validator: (value) {
        if (value == null || value.isEmpty) {
          return 'playlist title is required';
        }
        return null;
      },
    );

    if (!isFull) {
      return playNameWidget;
    }
    if (isFull && !state.fullImported) {
      return Material(
        color: EaseColors.light,
        borderRadius: BorderRadius.circular(6.0),
        child: InkWell(
          onTap: () {
            routerService.goCreatePlaylistEntries();
          },
          borderRadius: BorderRadius.circular(6.0),
          child: Container(
            padding: const EdgeInsets.symmetric(vertical: 30, horizontal: 30),
            child: const Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                EaseIcon(
                  color: EaseColors.primaryText,
                  iconToken: EaseIconsTokens.download,
                  size: 20,
                ),
                SizedBox(height: 10),
                Text(
                  "Import music and covers from storage",
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    color: EaseColors.primaryText,
                    fontSize: 12,
                  ),
                )
              ],
            ),
          ),
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        EaseFormInfo(
          label: "Import Info",
          child: Row(
            children: [
              Text(
                "${state.musicCount}",
                style: const TextStyle(
                  fontSize: 12,
                  color: EaseColors.primaryText,
                ),
              ),
              const Text(
                " musics",
                style: TextStyle(
                  fontSize: 12,
                  color: EaseColors.primaryText,
                ),
              ),
            ],
          ),
        ),
        gapWidget,
        playNameWidget,
        const SizedBox(height: 4),
        Wrap(
          children: state.recommendPlaylistNames.map((e) {
            var text = e;
            return ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 200),
              child: EaseTextButton(
                onPressed: () {
                  _titleController.text = text;
                },
                text: text,
                small: true,
                style: EaseTextButtonStyle.Default,
              ),
            );
          }).toList(),
        ),
        gapWidget,
        EaseFormImage(
          label: "Cover (Optional)",
          value: picture,
          onTapAdd: () {
            routerService.goCreatePlaylistCover();
          },
          onTapClear: () {
            bridge.scope((api) => api.clearCreatePlaylistCover());
          },
        ),
      ],
    );
  }
}

class CreatePlaylistNav extends StatelessWidget {
  final String text;
  final bool isActive;
  final double width;
  final void Function() onTapInactive;
  const CreatePlaylistNav({
    super.key,
    required this.text,
    required this.isActive,
    required this.width,
    required this.onTapInactive,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: () {
        if (!isActive) {
          onTapInactive();
        }
      },
      child: SizedBox(
        width: width,
        child: Column(
          children: [
            Text(
              text,
              style: TextStyle(
                color: isActive
                    ? EaseColors.primaryText
                    : EaseColors.secondaryText,
                fontSize: 12,
              ),
            ),
            if (isActive)
              FractionallySizedBox(
                widthFactor: 0.5,
                child: Container(
                  height: 1,
                  color: EaseColors.primaryText,
                ),
              ),
          ],
        ),
      ),
    );
  }
}
