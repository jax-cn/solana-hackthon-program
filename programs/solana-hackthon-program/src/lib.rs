use anchor_lang::prelude::*;
use ed25519_dalek::{Verifier, Signature as Ed25519Signature, PublicKey as Ed25519PublicKey};
use anchor_spl::token::{self, TokenAccount};

declare_id!("2RKBndJAMhMAq2E5bZyDGQsKDcuTkWAGTTBfmrhbNxG2");

#[error_code]
pub enum CustomError {
    InsufficientFunds,
}

#[program]
pub mod solana_hackthon_program {
    use std::{io::Read, ops::DerefMut};

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn register_gpu_node(ctx: Context<RegisterGPUNode>, node: GPUNode) -> Result<()> {
        let gpu_node_account = &mut ctx.accounts.gpu_node;
        let gpu_node_data = gpu_node_account.deref_mut();
        *gpu_node_data = node;

        // check if the node already exists, don't consider transfer of node ownership
        for n in &mut ctx.accounts.gpu_nodes.nodes {
            if n.to_bytes() == gpu_node_account.key().to_bytes() {
                return Ok(());
            }
        }
        // if the node does not exist, push it to the user list & global registry
        ctx.accounts.gpu_nodes.nodes.push(gpu_node_account.key());
        ctx.accounts.gpu_node_registry.nodes.push(gpu_node_account.key());
        Ok(())
    }

    pub fn register_agent(ctx: Context<RegisterAgent>, agent: Agent) -> Result<()> {
        let agent_account = &mut ctx.accounts.agent;
        let agent_data = agent_account.deref_mut();
        *agent_data = agent;

        // check if the node already exists, don't consider transfer of node ownership
        for a in &mut ctx.accounts.agent_registry.agents {
            if a.to_bytes() == agent_account.key().to_bytes() {
                return Ok(());
            }
        }
        // if the agent does not exist, push it to the user list & global registry
        ctx.accounts.agent_list.agents.push(agent_account.key());
        ctx.accounts.agent_registry.agents.push(agent_account.key());
        Ok(())
    }

    pub fn submit_task(ctx: Context<SubmitTask>, task: AiTask, signature: AiTaskSignature) -> Result<()> {
        // TODO: check agent/ exists
        // verify signature
        let sig_message = task.try_to_vec().unwrap();
        Ed25519PublicKey::from_bytes(&task.user.to_bytes()).unwrap().verify(&sig_message, &Ed25519Signature::from_bytes(&signature.user).unwrap()).unwrap();
        Ed25519PublicKey::from_bytes(&task.user.to_bytes()).unwrap().verify(&sig_message, &Ed25519Signature::from_bytes(&signature.agent).unwrap()).unwrap();
        Ed25519PublicKey::from_bytes(&task.user.to_bytes()).unwrap().verify(&sig_message, &Ed25519Signature::from_bytes(&signature.gpu_node).unwrap()).unwrap();

        // verify user balance
        let delegate_account_info = ctx.accounts.delegate.to_account_info();
        let delegate_account_data = delegate_account_info.try_borrow_data()?;
        let delegate_account = TokenAccount::try_deserialize(&mut delegate_account_data.as_ref())?;

        let approved_amount = delegate_account.amount;
        if (approved_amount < task.price) {
            return err!(CustomError::InsufficientFunds);
        }
        
        // transfer funds to agent
        let price = task.price;
        let agent_reward = ((price as f64) * ctx.accounts.agent.revenue_split) as u64;
        let gpu_node_reward = price - agent_reward;
        anchor_spl::token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), token::Transfer {
            from: ctx.accounts.delegate.to_account_info(),
            to: ctx.accounts.agent.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        }), agent_reward).unwrap();
        anchor_spl::token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), token::Transfer {
            from: ctx.accounts.delegate.to_account_info(),
            to: ctx.accounts.gpu_node.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        }), gpu_node_reward).unwrap();
        Ok(())
    }


}


#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, seeds = [b"gpu_node_registry"], bump, space = 8 + (99999 * 32))]
    pub gpu_node_registry: Account<'info, GPUNodeRegistry>,
    #[account(init, payer = user, seeds = [b"agent_registry"], bump, space = 8 + (9999 * 32))]
    pub agent_registry: Account<'info, AgentRegistry>,
    #[account(init, payer = user, seeds = [b"ai_task_registry"], bump, space = 8 + (9999 * 32))]
    pub ai_task_registry: Account<'info, AiTaskRegistry>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}


#[account]
#[derive(InitSpace, Default)]
pub struct GPUNode {
    #[max_len(32)]
    pub id: String,
    pub owner: Pubkey,
    #[max_len(128)]
    pub cards: Vec<Card>,
    #[max_len(16)]
    pub cuda_version: String,
    pub price: u64,
    #[max_len(256)]
    pub endpoint: String, // ip or domain
}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct Card {
    #[max_len(64)]
    pub name: String,
    pub memory: u32, // MB
}

#[account]
pub struct GPUNodeList {
    pub nodes: Vec<Pubkey>,
}

#[account]
pub struct GPUNodeRegistry {
    pub nodes: Vec<Pubkey>,
}

#[derive(Accounts)]
pub struct RegisterGPUNode<'info> {
    #[account(mut)]
    pub gpu_node_registry: Account<'info, GPUNodeRegistry>,
    #[account(init, payer = owner, space = 8 + 9999 * 32, seeds = [b"gpu_nodes", owner.key().as_ref()], bump)]
    pub gpu_nodes: Account<'info, GPUNodeList>,
    #[account(init, payer = owner, space = 8 + GPUNode::INIT_SPACE)]
    pub gpu_node: Account<'info, GPUNode>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}


#[account]
#[derive(InitSpace, Default)]
pub struct Agent {
    pub owner: Pubkey,
    #[max_len(64)]
    pub title: String,
    #[max_len(2048)]
    pub desc: String,
    #[max_len(4096)]
    pub poster: String, // image link
    #[max_len(8)]
    pub category: String, // LLM,Image,Audio,Video
    #[max_len(4096)]
    pub docker_image_href: String, // http link
    #[max_len(8)]
    pub api_protocol: String, // https,wss
    pub api_port: u16,
    #[max_len(4096)]
    pub api_doc: String, // http link
    pub revenue_split: f64,
}

#[account]
pub struct AgentList {
    pub agents: Vec<Pubkey>,
}

#[account]
pub struct AgentRegistry {
    pub agents: Vec<Pubkey>,
}

#[derive(Accounts)]
pub struct RegisterAgent<'info> {
    #[account(mut)]
    pub agent_registry: Account<'info, AgentRegistry>,
    #[account(init, payer = owner, space = 8 + 999 * 32, seeds = [b"agents", owner.key().as_ref()], bump)]
    pub agent_list: Account<'info, AgentList>,
    #[account(init, payer = owner, space = 8 + Agent::INIT_SPACE)]
    pub agent: Account<'info, Agent>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}


#[account]
#[derive(InitSpace, Default)]
pub struct AiTask {
    pub user: Pubkey,
    pub agent_owner: Pubkey,
    pub gpu_node_owner: Pubkey,
    pub timestamp: u64,
    pub price: u64,
    // #[max_len(256)]
    // pub params_hash: String,
    // #[max_len(4096)]
    // pub endpoint: String, // endpoint + port
    // #[max_len(2048)]
    // pub proof_of_work: String, // max length 2KB
}

#[account]
pub struct AiTaskRegistry {
    pub tasks: Vec<Pubkey>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AiTaskSignature {
    pub user: Vec<u8>,
    pub agent: Vec<u8>,
    pub gpu_node: Vec<u8>,
}


#[derive(Accounts)]
pub struct SubmitTask<'info> {
    #[account(executable)]
    token_program: AccountInfo<'info>,
    #[account(mut)]
    pub ai_task_registry: Account<'info, AiTaskRegistry>,
    #[account(init, payer = user, space = 8 + AiTask::INIT_SPACE, owner = system_program.key())]
    pub ai_task: Account<'info, AiTask>,
    #[account(owner = ai_task.gpu_node_owner)]
    pub gpu_node: Account<'info, GPUNode>,
    #[account(owner = ai_task.agent_owner)]
    pub agent: Account<'info, Agent>,
    #[account(seeds = [user.key().as_ref()], bump)]
    pub delegate: AccountInfo<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}