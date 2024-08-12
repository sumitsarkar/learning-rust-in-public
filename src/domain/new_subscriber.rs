use crate::domain::subscriber_name::SubscriberName;

use super::subscriber_email::SubscriberEmail;

pub struct NewSubscriber {
    pub email: SubscriberEmail,
    pub name: SubscriberName,
}
