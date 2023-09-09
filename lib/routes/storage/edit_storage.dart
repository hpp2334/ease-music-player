import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/edit_storage.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:ease_music_player/widgets/dialog.dart';
import 'package:ease_music_player/widgets/form.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:ease_music_player/widgets/screen_container.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../services/router.service.dart';

class StorageSelectItemWidget extends StatelessWidget {
  final StorageType storageType;
  final String label;
  final EaseIconsToken iconToken;
  final bool active;
  final void Function() onTap;

  const StorageSelectItemWidget({
    super.key,
    required this.storageType,
    required this.label,
    required this.iconToken,
    required this.active,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      color: active ? EaseColors.primary : EaseColors.light,
      borderRadius: BorderRadius.circular(20),
      child: InkWell(
        borderRadius: BorderRadius.circular(20),
        onTap: onTap,
        child: SizedBox(
          width: 100,
          height: 100,
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              EaseIcon(
                color: active ? EaseColors.surface : EaseColors.primaryText,
                iconToken: iconToken,
                size: 32,
              ),
              Text(
                label,
                style: TextStyle(
                  color: active ? EaseColors.surface : EaseColors.primaryText,
                  fontSize: 14,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class StorageSelectWidget extends StatelessWidget {
  final StorageType value;
  final void Function(StorageType) onChange;

  const StorageSelectWidget({
    super.key,
    required this.value,
    required this.onChange,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        StorageSelectItemWidget(
          active: value == StorageType.Webdav,
          storageType: StorageType.Webdav,
          label: "WebDAV",
          iconToken: EaseIconsTokens.cloud,
          onTap: () {
            onChange(StorageType.Webdav);
          },
        ),
      ],
    );
  }
}

class EditStoragePage extends StatefulWidget {
  const EditStoragePage({
    super.key,
  });

  @override
  EditStoragePageState createState() => EditStoragePageState();
}

class EditStoragePageState extends State<EditStoragePage> {
  final TextEditingController _usernameController = TextEditingController();
  final TextEditingController _passwordController = TextEditingController();
  final TextEditingController _addrController = TextEditingController();
  final TextEditingController _aliasController = TextEditingController();
  bool _isAnonymous = false;
  StorageType _storageType = StorageType.Webdav;
  StorageId? _id;
  final GlobalKey<FormState> _formKey = GlobalKey();

  int _preparedSignal = 0;
  EditStorageModel? model;

  @override
  void initState() {
    super.initState();
    reInitValues();

    model = context.read<EditStorageModel>();
    model?.addListener(prepareSignalListener);
    prepareSignalListener();
  }

  @override
  void dispose() {
    model?.removeListener(prepareSignalListener);
    _usernameController.dispose();
    _passwordController.dispose();
    _addrController.dispose();
    super.dispose();
  }

  void prepareSignalListener() {
    final values = context.read<EditStorageModel>().value;
    if (values.updateSignal != _preparedSignal) {
      _preparedSignal = values.updateSignal;
      reInitValues();
    }
  }

  void reInitValues() {
    final values = context.read<EditStorageModel>().value;
    _id = values.info.id;
    _usernameController.text = values.info.username;
    _passwordController.text = values.info.password;
    _addrController.text = values.info.addr;
    _aliasController.text = values.info.alias ?? "";
    _isAnonymous = values.info.isAnonymous;
    _storageType = values.info.typ;
  }

  ArgUpsertStorage? buildArg() {
    if (_formKey.currentState?.validate() != true) {
      return null;
    }
    String username = _usernameController.text;
    String password = _passwordController.text;
    bool isAnonymous = _isAnonymous;
    String addr = _addrController.text;
    String alias = _aliasController.text;
    StorageType typ = _storageType;
    return ArgUpsertStorage(
      id: _id,
      alias: alias,
      addr: addr,
      username: username,
      password: password,
      isAnonymous: isAnonymous,
      typ: typ,
    );
  }

  void _onSubmit() {
    final arg = buildArg();
    if (arg == null) {
      return;
    }

    bridge.scope((api) => api.upsertStorage(
          arg: arg,
        ));
    routerService.back();
  }

  @override
  Widget build(BuildContext context) {
    final values = context.watch<EditStorageModel>().value;
    return EaseScreenContainer(
      hidePlayer: true,
      header: PreferredSize(
        preferredSize: const Size.fromHeight(62),
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              EaseIconButton(
                iconColor: EaseColors.primaryText,
                iconToken: EaseIconsTokens.back,
                size: EaseIconButtonSize.Small,
                onTap: (_) {
                  routerService.back();
                },
              ),
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  if (!values.isCreated) const _RemoveStorageButton(),
                  _TestConnectionButton(
                    buildArg: buildArg,
                  ),
                  EaseIconButton(
                    iconColor: EaseColors.primaryText,
                    iconToken: EaseIconsTokens.ok,
                    size: EaseIconButtonSize.Small,
                    onTap: (_) => _onSubmit(),
                  ),
                ],
              )
            ],
          ),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 30),
        child: SingleChildScrollView(
          child: Form(
            key: _formKey,
            autovalidateMode: AutovalidateMode.disabled,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                StorageSelectWidget(
                  value: _storageType,
                  onChange: (v) {
                    setState(() {
                      _storageType = v;
                    });
                  },
                ),
                const SizedBox(height: 40),
                EaseFormSwitch(
                  label: "Anonymous",
                  value: _isAnonymous,
                  onChange: (value) {
                    setState(() {
                      _isAnonymous = value;
                    });
                  },
                ),
                const SizedBox(height: 20),
                EaseFormText(
                  controller: _aliasController,
                  label: 'Alias',
                ),
                const SizedBox(height: 20),
                EaseFormText(
                  controller: _addrController,
                  label: 'Address',
                  validator: (value) => value == null || value.isEmpty
                      ? "Address should not be empty"
                      : null,
                ),
                const SizedBox(height: 20),
                if (!_isAnonymous)
                  EaseFormText(
                    controller: _usernameController,
                    label: 'Username',
                    validator: (value) => value == null || value.isEmpty
                        ? "Username should not be empty"
                        : null,
                  ),
                if (!_isAnonymous) const SizedBox(height: 20),
                if (!_isAnonymous)
                  EaseFormText(
                    controller: _passwordController,
                    label: 'Password',
                    obscureText: true,
                    validator: (value) => value == null || value.isEmpty
                        ? "Password should not be empty"
                        : null,
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _RemoveStorageButton extends StatelessWidget {
  const _RemoveStorageButton();

  static const boldTextStyle = TextStyle(
    fontSize: 14,
    fontWeight: FontWeight.bold,
    color: EaseColors.primaryText,
    height: 1.5,
  );

  static const normalTextStyle = TextStyle(
    fontSize: 14,
    color: EaseColors.primaryText,
    height: 1.5,
  );

  @override
  Widget build(BuildContext context) {
    final state = context.read<EditStorageModel>().value;

    final hasAnyMusicOrPlaylist =
        state.musicCount > 0 || state.playlistCount > 0;

    return EaseIconButton(
      iconToken: EaseIconsTokens.deleteSleep,
      onTap: (_) {
        showConfirmDialog(
          context,
          () {
            bridge.scope((api) => api.removeStorage(arg: state.info.id!));
            routerService.back();
          },
          (_) => Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              RichText(
                text: TextSpan(children: [
                  const TextSpan(
                    text: "Are you sure to remove storage \"",
                    style: normalTextStyle,
                  ),
                  TextSpan(
                    text: state.title,
                    style: boldTextStyle,
                  ),
                  const TextSpan(
                    text: "\"?",
                    style: normalTextStyle,
                  ),
                ]),
              ),
              if (!hasAnyMusicOrPlaylist)
                const Text(
                  "No music will be removed.",
                  style: normalTextStyle,
                ),
              if (hasAnyMusicOrPlaylist)
                RichText(
                  text: TextSpan(children: [
                    TextSpan(
                      text: "${state.musicCount}",
                      style: boldTextStyle,
                    ),
                    const TextSpan(
                      text: " musics will be removed in ",
                      style: normalTextStyle,
                    ),
                    TextSpan(
                      text: "${state.playlistCount}",
                      style: boldTextStyle,
                    ),
                    const TextSpan(
                      text: " playlists.",
                      style: normalTextStyle,
                    ),
                  ]),
                ),
            ],
          ),
        );
      },
      size: EaseIconButtonSize.Small,
      iconColor: EaseColors.error,
    );
  }
}

class _TestConnectionButton extends StatelessWidget {
  final ArgUpsertStorage? Function() buildArg;

  const _TestConnectionButton({
    required this.buildArg,
  });

  void handleTap() {
    final arg = buildArg();
    if (arg != null) {
      bridge.scope((api) => api.testConnection(arg: arg));
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = context.watch<EditStorageModel>().value;

    var color = EaseColors.primaryText;
    var disabled = false;

    if (state.test == StorageConnectionTestResult.Success) {
      color = EaseColors.success;
    } else if (state.test == StorageConnectionTestResult.Timeout) {
      color = EaseColors.error;
    } else if (state.test == StorageConnectionTestResult.Unauthorized) {
      color = EaseColors.error;
    } else if (state.test == StorageConnectionTestResult.OtherError) {
      color = EaseColors.error;
    } else if (state.test == StorageConnectionTestResult.Testing) {
      color = EaseColors.primaryLight;
      disabled = true;
    }

    return EaseIconButton(
      iconColor: color,
      iconToken: EaseIconsTokens.wifiTethering,
      size: EaseIconButtonSize.Small,
      disabled: disabled,
      onTap: (_) => handleTap(),
    );
  }
}
