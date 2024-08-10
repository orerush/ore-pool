use ore_api::*;
use ore_pool_api::{consts::*, instruction::*, loaders::*};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    {self},
};

/// Open ...
pub fn process_open<'a, 'info>(accounts: &'a [AccountInfo<'info>], data: &[u8]) -> ProgramResult {
    // Load accounts.
    let [signer, authority_info, member_info, pool_info, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    // TODO Account loaders
    // load_operator(signer)?;
    // load_any(miner_info)?;
    // load_uninitialized_pda(pool_info, &[POOL], args.pool_bump, &ore_pool_api::id())?;
    load_program(system_program, system_program::id())?;

    // Initialize member account
    create_pda(
        member_info,
        &ore_pool_api::id(),
        8 + size_of::<Member>(),
        &[MEMBER, authority_info.key.as_ref(), &[args.member_bump]],
        system_program,
        signer,
    )?;
    let mut member_data = member_info.try_borrow_mut_data()?;
    member_data[0] = Member::discriminator() as u8;
    let member = Member::try_from_bytes_mut(&mut member_data)?;
    member.authority = authority_info.key;
    member.balance = 0;

    // Update member count
    let mut pool_data = pool_info.try_borrow_mut_data()?;
    let pool = Pool::try_from_bytes_mut(&mut pool_data)?;
    pool.total_members = pool.total_members.checked_add(1).unwrap();

    Ok(())
}