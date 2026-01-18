cool.

so before we can build events and animations and stuff, 
we need 

a CollisionSystem,
a CollisionComponent  that has modes/ types:
CollisionComponent::STATIC()    // non movable, causes collision only
CollisionComponent::KINEMATIC() // movable, controlled by physics simulation
CollisionComponent::RIGGED()    // camera control, scripting etc
This needs to happen in a separate thread that CollisionSystem manages using message passing to and from that thread:
enum CollisionMessage {
    // to thread
    TICK
    ADD_OBJECT
    REMOVE_OBJECT
    UPDATE_OBJECT

    // from thread
    COLLISION_DETECTED
}

PhysicsSystem comes after this.


and.... to store stuff in the CollisionSystem thread, 
we probably want bvh i think.
What do you think?