import "misty-ui-prelude"
import type React from 'react'
import Reconciler, { type HostConfig } from 'react-reconciler'
import { Element } from "misty-ui-core"
import { getRootElement } from "./global";

type Type = string;
type Props = any;
type Container = Element;
type Instance = Element;
type TextInstance = never;
type SuspenseInstance = never;
type HydratableInstance = never;
type PublicInstance = {};
type HostContext = {};
type UpdatePayload = {};
type ChildSet = {};
type TimeoutHandle = number;
type NoTimeout = 0;

const Host: HostConfig<
    Type,
    Props,
    Container,
    Instance,
    TextInstance,
    SuspenseInstance,
    HydratableInstance,
    PublicInstance,
    HostContext,
    UpdatePayload,
    ChildSet,
    TimeoutHandle,
    NoTimeout
> = {
    supportsMutation: true,
    supportsPersistence: false,
    supportsHydration: false,
    noTimeout: 0,
    isPrimaryRenderer: true,
    createInstance: function (type, props, rootContainer, hostContext, internalHandle) {
        throw new Error('Function not implemented.');
    },
    createTextInstance: function (text, rootContainer, hostContext, internalHandle) {
        throw new Error(`createTextInstance not implemented.`)
    },
    appendInitialChild: function (parentInstance, child) {
        throw new Error('Function not implemented.');
    },
    finalizeInitialChildren: function (instance, type, props, rootContainer, hostContext) {
        throw new Error('Function not implemented.');
    },
    prepareUpdate: function (instance, type, oldProps, newProps, rootContainer, hostContext) {
        throw new Error('Function not implemented.');
    },
    shouldSetTextContent: function (type, props) {
        throw new Error('Function not implemented.');
    },
    getRootHostContext: function (rootContainer) {
        throw new Error('Function not implemented.');
    },
    getChildHostContext: function (parentHostContext, type, rootContainer) {
        throw new Error('Function not implemented.');
    },
    getPublicInstance: function (instance) {
        throw new Error('Function not implemented.');
    },
    prepareForCommit: function (containerInfo) {
        throw new Error('Function not implemented.');
    },
    resetAfterCommit: function (containerInfo) {
        throw new Error('Function not implemented.');
    },
    preparePortalMount: function (containerInfo) {
        throw new Error('Function not implemented.');
    },
    scheduleTimeout: function (fn, delay) {
        throw new Error('Function not implemented.');
    },
    cancelTimeout: function (id) {
        throw new Error('Function not implemented.');
    },
    getCurrentEventPriority: function (){
        throw new Error('Function not implemented.');
    },
    getInstanceFromNode: function (node) {
        throw new Error('Function not implemented.');
    },
    beforeActiveInstanceBlur: function () {
        throw new Error('Function not implemented.');
    },
    afterActiveInstanceBlur: function () {
        throw new Error('Function not implemented.');
    },
    prepareScopeUpdate: function (scopeInstance, instance) {
        throw new Error('Function not implemented.');
    },
    getInstanceFromScope: function (scopeInstance) {
        throw new Error('Function not implemented.');
    },
    detachDeletedInstance: function (node) {
        throw new Error('Function not implemented.');
    },
}

const MistyRenderer = Reconciler(Host)

export const MistyUI = {
    render(element: React.ReactNode) {
        MistyRenderer.updateContainer(element, getRootElement())
    }
}