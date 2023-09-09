import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/models/edit_playlist.dart';
import 'package:ease_music_player/services/resource.service.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/widgets/form.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../widgets/button.dart';
import '../../widgets/dialog.dart';

void showEditPlaylistDialog(BuildContext context, PlaylistId id) {
  bridge.scope((api) => api.prepareEditPlaylist(arg: id));
  showDialog(
    context: context,
    builder: (BuildContext context) {
      return const _EditPlaylistDialog();
    },
  );
}

class _EditPlaylistDialog extends StatefulWidget {
  const _EditPlaylistDialog();

  @override
  _EditPlaylistDialogState createState() => _EditPlaylistDialogState();
}

class _EditPlaylistDialogState extends State<_EditPlaylistDialog> {
  final _formKey = GlobalKey<FormState>();
  int _lastSignal = 0;
  TextEditingController _titleController = TextEditingController();

  EditPlaylistModel? model;

  @override
  void initState() {
    super.initState();

    model = context.read<EditPlaylistModel>();

    final editPlaylistValues = model!.value;
    _lastSignal = editPlaylistValues.preparedSignal;
    _titleController = TextEditingController(text: editPlaylistValues.name);

    model!.addListener(preparedSignalListener);
    _titleController.addListener(() {
      bridge.scope(
          (api) => api.updateEditPlaylistName(arg: _titleController.text));
    });
  }

  @override
  void dispose() {
    model?.removeListener(preparedSignalListener);
    _titleController.dispose();
    bridge.scope((api) => api.clearEditPlaylistState());
    super.dispose();
  }

  void preparedSignalListener() {
    final model = context.read<EditPlaylistModel>();
    final editPlaylistValues = model.value;
    if (_lastSignal != editPlaylistValues.preparedSignal) {
      _lastSignal = editPlaylistValues.preparedSignal;
      _titleController.text = editPlaylistValues.name;
    }
  }

  @override
  Widget build(BuildContext context) {
    final editPlaylistValues = context.watch<EditPlaylistModel>().value;
    final picture = editPlaylistValues.picture != null
        ? resourceService.load(editPlaylistValues.picture!)
        : null;

    return EaseDialog(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 32, horizontal: 30),
            child: Form(
              key: _formKey,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  EaseFormText(
                    controller: _titleController,
                    label: "Playlist Name",
                    validator: (value) {
                      if (value == null || value.isEmpty) {
                        return 'playlist title is required';
                      }
                      return null;
                    },
                  ),
                  const SizedBox(
                    height: 24,
                  ),
                  EaseFormImage(
                    label: "Cover (Optional)",
                    value: picture,
                    onTapAdd: () {
                      routerService.goEditPlaylistCover();
                    },
                    onTapClear: () {
                      bridge.scope((api) => api.clearEditPlaylistCover());
                    },
                  ),
                ],
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 18, horizontal: 14),
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

                    bridge.scope((api) => api.finishEditPlaylist());
                    routerService.back();
                  },
                  text: 'OK',
                ),
              ],
            ),
          )
        ],
      ),
    );
  }
}
