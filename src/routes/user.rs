use actix_web::{get, http::StatusCode, patch, post, put, web, HttpResponse, Responder, Scope};
use argon2::{self, Argon2, PasswordHash, PasswordVerifier};
use lettre::AsyncTransport;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, ModelTrait, QueryFilter, Set};

use crate::{
    database::entity::{users, verifications},
    models::{MessageResponse, Response, UpdateUserSettings, UserCreateForm, UserData},
    state::State,
    util::{
        self,
        auth::{auth_role, Auth},
        random_string,
        user::{new_password, validate_username, verification_email},
        EMAIL_REGEX,
    },
};

pub fn get_routes(smtp_verification: bool) -> Scope {
    let scope = web::scope("/user")
        .service(create)
        .service(settings)
        .service(info);

    if smtp_verification {
        scope.service(resend_verify).service(verify)
    } else {
        scope
    }
}

#[get("")]
async fn info(auth: Auth<auth_role::User, true, true>) -> impl Responder {
    HttpResponse::Ok().json(UserData::from(auth.user))
}

#[put("/settings")]
async fn settings(
    auth: Auth<auth_role::User, true, true>,
    state: web::Data<State>,
    form: web::Json<UpdateUserSettings>,
) -> Response<impl Responder> {
    // Check if the users password is correct
    if !Argon2::default()
        .verify_password(
            form.current_password.as_bytes(),
            &PasswordHash::new(&auth.user.password)?,
        )
        .is_ok()
    {
        return Ok(
            MessageResponse::new(StatusCode::BAD_REQUEST, "Incorrect current password")
                .http_response(),
        );
    }

    // We set the properties in here (except current_password) from the if blocks.
    // So that if we get one error afterwards it does not change partial data
    let mut to_change = UpdateUserSettings {
        current_password: "".to_string(), // Don't change this

        email: None,
        username: None,
        new_password: None,
    };

    if let Some(new_password) = &form.new_password {
        to_change.new_password = match util::user::new_password(&new_password)? {
            Ok(v) => Some(v),
            Err(err) => return Ok(err.http_response()),
        }
    }

    if let Some(new_email) = &form.email {
        if !EMAIL_REGEX.is_match(&new_email) {
            return Ok(
                MessageResponse::new(StatusCode::BAD_REQUEST, "Invalid email was provided")
                    .http_response(),
            );
        }

        if users::Entity::find()
            .filter(users::Column::Email.eq(new_email.to_owned()))
            .one(&state.database)
            .await?
            .is_some()
        {
            return Ok(MessageResponse::new(
                StatusCode::CONFLICT,
                "An account with that email already exists!",
            )
            .http_response());
        }

        to_change.email = Some(new_email.to_string());
    }

    if let Some(new_username) = &form.username {
        if let Err(err) = validate_username(&new_username) {
            return Ok(err.http_response());
        }

        if users::Entity::find()
            .filter(users::Column::Username.eq(new_username.to_owned()))
            .one(&state.database)
            .await?
            .is_some()
        {
            return Ok(MessageResponse::new(
                StatusCode::CONFLICT,
                "An account with that username already exists!",
            )
            .http_response());
        }

        to_change.username = Some(new_username.to_string());
    }

    let mut update_model = users::ActiveModel {
        id: Set(auth.user.id.to_owned()),
        ..Default::default()
    };

    // Update email if change validated
    if let Some(email) = &to_change.email {
        update_model.email = Set(email.to_owned());

        if let Some(_) = &state.smtp_client {
            update_model.verified = Set(false)
        }
    }

    // Update password if change validated
    if let Some(new_password) = to_change.new_password {
        update_model.password = Set(new_password);
    }

    // Update username if change validated
    if let Some(new_username) = to_change.username {
        update_model.username = Set(new_username);
    }

    // Perform all updates
    update_model.update(&state.database).await?;

    // After the update we need to send the new verification email if the email was updated
    if let (Some(email), Some(smtp)) = (to_change.email, &state.smtp_client) {
        // If email validation is on we need to resend the email and unverify the user
        let random_code = random_string(72);

        let success = verifications::ActiveModel {
            user_id: Set(auth.user.id.to_owned()),
            code: Set(random_code.to_owned()),
            ..Default::default()
        }
        .insert(&state.database)
        .await
        .is_err();

        if success {
            let email =
                verification_email(&state.base_url.to_string(), &smtp.1, &email, &random_code);
            let mailer = smtp.clone().0;
            tokio::spawn(async move {
                let _ = mailer.send(email).await;
            });
        }
    }

    // Send updated user data in case of data change
    Ok(HttpResponse::Ok().json(UserData::from(
        users::Entity::find_by_id(auth.user.id)
            .one(&state.database)
            .await?
            .ok_or(DbErr::Custom(
                "user was not found even though we just did an update".to_string(),
            ))?,
    )))
}

#[post("")]
async fn create(
    state: web::Data<State>,
    form: web::Json<UserCreateForm>,
) -> Response<impl Responder> {
    // Check if username length is within bounds
    if let Err(err) = validate_username(&form.username) {
        return Ok(err);
    }

    if !EMAIL_REGEX.is_match(&form.email) {
        return Ok(MessageResponse::new(
            StatusCode::BAD_REQUEST,
            "Invalid email was provided",
        ));
    }

    // Check if user with same email was found
    if users::Entity::find()
        .filter(users::Column::Email.eq(form.email.to_owned()))
        .one(&state.database)
        .await?
        .is_some()
    {
        return Ok(MessageResponse::new(
            StatusCode::CONFLICT,
            "An account with that email already exists!",
        ));
    }

    // Check if user with same username was found
    if users::Entity::find()
        .filter(users::Column::Username.eq(form.username.to_owned()))
        .one(&state.database)
        .await?
        .is_some()
    {
        return Ok(MessageResponse::new(
            StatusCode::CONFLICT,
            "An account with that username already exists!",
        ));
    }

    let user_data: users::Model = users::ActiveModel {
        username: Set(form.username.to_owned()),
        email: Set(form.email.to_owned()),
        password: Set(match new_password(&form.password)? {
            Ok(password_hashed) => password_hashed,
            Err(err) => return Ok(err),
        }),
        ..Default::default()
    }
    .insert(&state.database)
    .await?;

    if let Some(smtp) = &state.smtp_client {
        let random_code = random_string(72);

        let success = verifications::ActiveModel {
            user_id: Set(user_data.id.to_owned()),
            code: Set(random_code.to_owned()),
            ..Default::default()
        }
        .insert(&state.database)
        .await
        .is_ok();

        if success {
            let email = verification_email(
                &state.base_url.to_string(),
                &smtp.1,
                &user_data.email,
                &random_code,
            );
            let mailer = smtp.clone().0;
            tokio::spawn(async move {
                let _ = mailer.send(email).await;
            });
        }
    }

    Ok(MessageResponse::new(
        StatusCode::OK,
        "User has successfully been created",
    ))
}

#[patch("/verify/resend")]
async fn resend_verify(
    state: web::Data<State>,
    auth: Auth<auth_role::User, true, false>,
) -> Response<impl Responder> {
    if auth.user.verified {
        return Ok(MessageResponse::new(
            StatusCode::CONFLICT,
            "You are already verified",
        ));
    }

    let mut verification_model = verifications::ActiveModel {
        user_id: Set(auth.user.id.to_owned()),
        ..Default::default()
    };

    verification_model.clone().delete(&state.database).await?;

    // Update model and create new verification
    let random_code = random_string(72);
    verification_model.code = Set(random_code.to_owned());
    verification_model.insert(&state.database).await?;

    let smtp = state.smtp_client.as_ref().unwrap();
    let email = verification_email(
        &state.base_url.to_string(),
        &smtp.1,
        &auth.user.email,
        &random_code,
    );

    let mailer = smtp.clone().0;
    tokio::spawn(async move {
        let _ = mailer.send(email).await;
    });

    Ok(MessageResponse::new(
        StatusCode::OK,
        &format!("Verification email resent to {}", auth.user.email),
    ))
}

#[patch("/verify/{code}")]
async fn verify(state: web::Data<State>, code: web::Path<String>) -> Response<impl Responder> {
    // let (verification, user): (verifications::Model, users::Model)
    match verifications::Entity::find()
        .filter(verifications::Column::Code.eq(code.to_owned()))
        .find_also_related(users::Entity)
        .one(&state.database)
        .await?
    {
        Some((verification, user_data_opt)) => {
            // This can't really be None
            let user_data = user_data_opt.unwrap();

            verification.delete(&state.database).await?;

            // This case can ONLY happen if SMTP verification is disabled, the user tries to access their account, and THEN re-enables
            if user_data.verified {
                return Ok(MessageResponse::new(
                    StatusCode::CONFLICT,
                    "User was already verified",
                ));
            }

            let mut active_user: users::ActiveModel = user_data.into();
            active_user.verified = Set(true);
            active_user.update(&state.database).await?;

            Ok(MessageResponse::new(
                StatusCode::OK,
                "User has been verified",
            ))
        }
        None => return Ok(MessageResponse::bad_request()),
    }
}

// This needs to delete every file owned by the user
// #[post("delete")]
// async fn delete(state: web::Data<State>, auth: auth::middleware::User, form: web::Json<UserDeleteForm>) -> impl Responder {
//     let matches = match argon2::verify_encoded(&auth.0.password, form.current_password.as_bytes()) {
//         Ok(matches) => matches,
//         Err(_) => return MessageResponse::internal_server_error()
//     };
// }
