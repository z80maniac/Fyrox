# rusty editor

Scene editor for rg3d engine. **It is not ready for use yet.**

## Motivation

rg3d engine getting bigger, but still does not have scene editor what makes creation of scenes harder - you have to use 3d editors (like Blender, 3ds Max, etc.) to create scenes in them, no need to say that this looks like "hack" instead of normal solution. This editor is planned to be relatively small; not tied to any type of game. It will be used to compose scene from existing 3d models, setup physics, and all such stuff.

## Controls

- [Click] - Select
- [W][S][A][D] - Move camera
- [1] - Move interaction mode
- [2] - Scale interaction mode
- [3] - Rotate interaction mode
- [Z] - Undo
- [Y] - Redo

## Plan

- [x] Interaction modes.
	- [x] Move.
	- [x] Scale.
	- [x] Rotate.
- [x] Undo/redo.
- [x] Camera controller.
- [x] Save scene.
- [x] Load scene.
- [ ] Commands
	- [x] Move.
	- [x] Scale.
	- [x] Rotate.
	- [x] Delete node.
	- [x] Create node.
	- [x] Link nodes.
	- [ ] Other?
- [ ] World outliner
	- [x] Syncing with graph.
	- [x] Syncing selection with scene selection and vice versa.
	- [x] Drag'n'drop hierarchy edit.
	- [ ] Nodes context menu
- [ ] Node properties editor
	- [ ] Base node
		- [x] Show name.
		- [ ] Edit name.
		- [ ] Edit position.
		- [ ] Edit rotation.
		- [ ] Edit scale.
	- [ ] Light node
	- [ ] Particle system node.
		- [ ] Particle system properties.
	- [ ] Sprite node.
		- [ ] Sprite properties.
	- [ ] Mesh node.
		- [ ] Mesh properties.
- [ ] Asset browser.

... Lots of stuff.